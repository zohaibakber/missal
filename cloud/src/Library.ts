import { and, desc, eq, sql } from "drizzle-orm";
import * as Context from "effect/Context";
import * as SecureRandom from "effect/crypto/SecureRandom";
import * as DateTime from "effect/DateTime";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import * as Option from "effect/Option";
import * as Schema from "effect/Schema";
import {
  type Entry,
  ManagedTemplate,
  PublishConflict,
  TemplateNotFound,
  TemplateVersion,
  type Upload,
} from "./Api.ts";
import { Blobs } from "./Blobs.ts";
import { Atomic, Db } from "./Database.ts";
import { storage } from "./Failures.ts";
import { Template, TemplateId, type UserId } from "./Models.ts";
import { templates, templateVersions, users } from "./Schema.ts";

const objectKey = (id: string, version: number, unique: string) =>
  `templates/${id}/${version}-${unique}.msb`;

const notFound = () => new TemplateNotFound({ error: "not found" });

const conflict = () =>
  new PublishConflict({
    error: "someone else published this template at the same time; try again",
  });

const toEntry = (template: Template): Entry => template;

interface NextVersion {
  readonly name: string;
  readonly fieldCount: number;
  readonly preview: string;
  readonly size: number;
  readonly objectKey: string;
  readonly restoredFrom: number | null;
}

const decodeTemplate = Schema.decodeUnknownEffect(Template);
const decodeTemplates = Schema.decodeUnknownEffect(Schema.Array(Template));
const decodeManaged = Schema.decodeUnknownEffect(Schema.Array(ManagedTemplate));
const decodeVersions = Schema.decodeUnknownEffect(Schema.Array(TemplateVersion));

const versionKey = (id: string, version: number) =>
  and(eq(templateVersions.templateId, id), eq(templateVersions.version, version));

export class Library extends Context.Service<Library>()("Library", {
  make: Effect.gen(function* () {
    const db = yield* Db;
    const atomic = yield* Atomic;
    const blobs = yield* Blobs;
    const random = yield* SecureRandom.SecureRandom;

    const storedVersion = (id: string, version: number) =>
      db
        .select({
          name: templateVersions.name,
          fieldCount: templateVersions.fieldCount,
          preview: templateVersions.preview,
          size: templateVersions.size,
          objectKey: templateVersions.objectKey,
        })
        .from(templateVersions)
        .where(versionKey(id, version))
        .pipe(
          Effect.map((rows) => Option.fromNullishOr(rows[0])),
          storage("read the version history"),
        );

    const find = (id: string) =>
      db
        .select()
        .from(templates)
        .where(eq(templates.id, id))
        .pipe(
          Effect.flatMap((rows) =>
            rows[0] === undefined ? Effect.succeedNone : Effect.asSome(decodeTemplate(rows[0])),
          ),
          storage("read the template"),
        );

    const list = db
      .select()
      .from(templates)
      .orderBy(templates.name)
      .pipe(
        Effect.flatMap(decodeTemplates),
        Effect.map((rows) => rows.map(toEntry)),
        storage("list the templates"),
        Effect.withSpan("Library.list"),
      );

    const managed = db
      .select({
        id: templates.id,
        name: templates.name,
        version: templates.version,
        fieldCount: templates.fieldCount,
        preview: templates.preview,
        size: templates.size,
        createdAt: templates.createdAt,
        updatedAt: templates.updatedAt,
        publisherName: users.name,
        publisherEmail: users.email,
        versionCount: db.$count(templateVersions, eq(templateVersions.templateId, templates.id)),
      })
      .from(templates)
      .leftJoin(users, eq(users.id, templates.publishedBy))
      .orderBy(templates.name)
      .pipe(
        Effect.flatMap(decodeManaged),
        storage("list the templates"),
        Effect.withSpan("Library.managed"),
      );

    const versions = Effect.fn("Library.versions")(function* (id: string) {
      const template = yield* find(id);
      if (Option.isNone(template)) return yield* notFound();
      return yield* db
        .select({
          version: templateVersions.version,
          name: templateVersions.name,
          fieldCount: templateVersions.fieldCount,
          size: templateVersions.size,
          publishedAt: templateVersions.publishedAt,
          restoredFrom: templateVersions.restoredFrom,
          publisherName: users.name,
          publisherEmail: users.email,
        })
        .from(templateVersions)
        .leftJoin(users, eq(users.id, templateVersions.publishedBy))
        .where(eq(templateVersions.templateId, id))
        .orderBy(desc(templateVersions.version))
        .pipe(Effect.flatMap(decodeVersions), storage("read the version history"));
    });

    const download = Effect.fn("Library.download")(function* (id: string) {
      const [current] = yield* db
        .select({ version: templateVersions.version, objectKey: templateVersions.objectKey })
        .from(templates)
        .innerJoin(
          templateVersions,
          and(
            eq(templateVersions.templateId, templates.id),
            eq(templateVersions.version, templates.version),
          ),
        )
        .where(eq(templates.id, id))
        .pipe(storage("read the template"));
      if (current === undefined) return yield* notFound();
      const { version, objectKey: key } = current;
      const content = yield* blobs.get(key).pipe(storage("read the template"));
      if (Option.isNone(content)) return yield* notFound();
      return { version, content: content.value };
    });

    const advance = Effect.fnUntraced(function* (current: Template, next: NextVersion, by: UserId) {
      const version = current.version + 1;
      const now = DateTime.toEpochMillis(yield* DateTime.now);
      const { objectKey: key, restoredFrom, name, fieldCount, preview, size } = next;
      const unchanged = and(eq(templates.id, current.id), eq(templates.version, current.version));
      yield* atomic
        .run([
          db.insert(templateVersions).select(
            db
              .select({
                templateId: templates.id,
                version: sql<number>`${version}`.as("version"),
                name: sql<string>`${name}`.as("name"),
                fieldCount: sql<number>`${fieldCount}`.as("field_count"),
                preview: sql<string>`${preview}`.as("preview"),
                size: sql<number>`${size}`.as("size"),
                objectKey: sql<string>`${key}`.as("object_key"),
                publishedBy: sql<string>`${by}`.as("published_by"),
                publishedAt: sql<number>`${now}`.as("published_at"),
                restoredFrom: sql<number | null>`${restoredFrom}`.as("restored_from"),
              })
              .from(templates)
              .where(unchanged),
          ),
          db
            .update(templates)
            .set({ name, version, fieldCount, preview, size, publishedBy: by, updatedAt: now })
            .where(unchanged),
        ])
        .pipe(storage("publish the template"));
      const [written] = yield* db
        .select({ objectKey: templateVersions.objectKey })
        .from(templateVersions)
        .where(versionKey(current.id, version))
        .pipe(storage("publish the template"));
      return written?.objectKey === key;
    });

    const create = Effect.fn("Library.create")(function* (upload: Upload, by: UserId) {
      const id = TemplateId.make(yield* random.uuidV4());
      const key = objectKey(id, 1, yield* random.uuidV4());
      yield* blobs.put(key, upload.content).pipe(storage("store the template"));
      const now = DateTime.toEpochMillis(yield* DateTime.now);
      const shared = {
        name: upload.name,
        version: 1,
        fieldCount: upload.fieldCount,
        preview: upload.preview,
        size: upload.content.length,
        publishedBy: by,
      };
      yield* atomic
        .run([
          db.insert(templates).values({ id, ...shared, createdAt: now, updatedAt: now }),
          db
            .insert(templateVersions)
            .values({ templateId: id, ...shared, objectKey: key, publishedAt: now }),
        ])
        .pipe(storage("publish the template"));
      return { id, version: 1 };
    });

    const update = Effect.fn("Library.update")(function* (id: string, upload: Upload, by: UserId) {
      const current = yield* find(id);
      if (Option.isNone(current)) return yield* notFound();
      const version = current.value.version + 1;
      const key = objectKey(id, version, yield* random.uuidV4());
      yield* blobs.put(key, upload.content).pipe(storage("store the template"));
      const written = yield* advance(
        current.value,
        {
          name: upload.name,
          fieldCount: upload.fieldCount,
          preview: upload.preview,
          size: upload.content.length,
          objectKey: key,
          restoredFrom: null,
        },
        by,
      );
      if (!written) {
        yield* blobs.remove(key).pipe(Effect.ignore);
        return yield* conflict();
      }
      return { id: current.value.id, version };
    });

    const restore = Effect.fn("Library.restore")(function* (
      id: string,
      version: number,
      by: UserId,
    ) {
      const current = yield* find(id);
      if (Option.isNone(current)) return yield* notFound();
      if (version === current.value.version) {
        return { id: current.value.id, version };
      }
      const target = yield* storedVersion(id, version);
      if (Option.isNone(target)) return yield* notFound();
      const written = yield* advance(current.value, { ...target.value, restoredFrom: version }, by);
      if (!written) return yield* conflict();
      return { id: current.value.id, version: current.value.version + 1 };
    });

    const remove = Effect.fn("Library.remove")(function* (id: string) {
      const template = yield* find(id);
      if (Option.isNone(template)) return yield* notFound();
      yield* atomic
        .run([
          db.delete(templateVersions).where(eq(templateVersions.templateId, id)),
          db.delete(templates).where(eq(templates.id, id)),
        ])
        .pipe(storage("remove the template"));
      yield* blobs.removePrefix(`templates/${id}/`).pipe(storage("remove the template"));
    });

    return { list, managed, versions, download, create, update, restore, remove };
  }),
}) {
  static readonly layer = Layer.effect(Library, Library.make);
}
