import type { RuntimeContext } from "alchemy/RuntimeContext";
import * as Context from "effect/Context";
import * as SecureRandom from "effect/crypto/SecureRandom";
import * as DateTime from "effect/DateTime";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import * as Option from "effect/Option";
import * as Schema from "effect/Schema";
import * as SqlClient from "effect/sql/SqlClient";
import * as SqlModel from "effect/sql/SqlModel";
import * as SqlSchema from "effect/sql/SqlSchema";
import {
  type Entry,
  ManagedTemplate,
  PublishConflict,
  type StorageFailure,
  TemplateNotFound,
  TemplateVersion,
  type Upload,
} from "./Api.ts";
import { Blobs } from "./Blobs.ts";
import { Atomic } from "./Database.ts";
import { storage } from "./Failures.ts";
import { Template, TemplateId, type UserId } from "./Models.ts";

const objectKey = (id: string, version: number, unique: string) =>
  `templates/${id}/${version}-${unique}.msb`;

const notFound = () => new TemplateNotFound({ error: "not found" });

const conflict = () =>
  new PublishConflict({
    error: "someone else published this template at the same time; try again",
  });

const toEntry = (template: Template): Entry => template;

const StoredVersion = Schema.Struct({
  name: Schema.String,
  fieldCount: Schema.Int,
  preview: Schema.String,
  size: Schema.Int,
  objectKey: Schema.String,
});

const CurrentObject = Schema.Struct({ version: Schema.Int, objectKey: Schema.String });

const VersionKey = Schema.Struct({ id: Schema.String, version: Schema.Int });

type NextVersion = typeof StoredVersion.Type & { readonly restoredFrom: number | null };

export interface Stored {
  readonly version: number;
  readonly content: Uint8Array;
}

export interface Published {
  readonly id: TemplateId;
  readonly version: number;
}

type Effectful<A, E> = Effect.Effect<A, E, RuntimeContext>;

export class Library extends Context.Service<
  Library,
  {
    readonly list: Effectful<ReadonlyArray<Entry>, StorageFailure>;
    readonly managed: Effectful<ReadonlyArray<ManagedTemplate>, StorageFailure>;
    readonly versions: (
      id: string,
    ) => Effectful<ReadonlyArray<TemplateVersion>, TemplateNotFound | StorageFailure>;
    readonly download: (id: string) => Effectful<Stored, TemplateNotFound | StorageFailure>;
    readonly create: (upload: Upload, by: UserId) => Effectful<Published, StorageFailure>;
    readonly update: (
      id: string,
      upload: Upload,
      by: UserId,
    ) => Effectful<Published, TemplateNotFound | PublishConflict | StorageFailure>;
    readonly restore: (
      id: string,
      version: number,
      by: UserId,
    ) => Effectful<Published, TemplateNotFound | PublishConflict | StorageFailure>;
    readonly remove: (id: string) => Effectful<void, TemplateNotFound | StorageFailure>;
  }
>()("Library") {
  static readonly layer = Layer.effect(
    Library,
    Effect.gen(function* () {
      const sql = yield* SqlClient.SqlClient;
      const atomic = yield* Atomic;
      const blobs = yield* Blobs;
      const random = yield* SecureRandom.SecureRandom;
      const templates = yield* SqlModel.makeRepository(Template, {
        tableName: "templates",
        spanPrefix: "Templates",
        idColumn: "id",
      });
      const all = SqlSchema.findAll({
        Request: Schema.Void,
        Result: Template,
        execute: () => sql`SELECT * FROM templates ORDER BY name`,
      });
      const managedRows = SqlSchema.findAll({
        Request: Schema.Void,
        Result: ManagedTemplate,
        execute: () => sql`
          SELECT t.id, t.name, t.version, t.field_count, t.preview, t.size, t.created_at,
            t.updated_at, u.name AS publisher_name, u.email AS publisher_email,
            (SELECT COUNT(*) FROM template_versions v WHERE v.template_id = t.id) AS version_count
          FROM templates t
          LEFT JOIN users u ON u.id = t.published_by
          ORDER BY t.name`,
      });
      const versionRows = SqlSchema.findAll({
        Request: Schema.String,
        Result: TemplateVersion,
        execute: (id) => sql`
          SELECT v.version, v.name, v.field_count, v.size, v.published_at, v.restored_from,
            u.name AS publisher_name, u.email AS publisher_email
          FROM template_versions v
          LEFT JOIN users u ON u.id = v.published_by
          WHERE v.template_id = ${id}
          ORDER BY v.version DESC`,
      });
      const storedVersion = SqlSchema.findOneOption({
        Request: VersionKey,
        Result: StoredVersion,
        execute: ({ id, version }) => sql`
          SELECT name, field_count, preview, size, object_key
          FROM template_versions
          WHERE template_id = ${id} AND version = ${version}`,
      });
      const currentObject = SqlSchema.findOneOption({
        Request: Schema.String,
        Result: CurrentObject,
        execute: (id) => sql`
          SELECT v.version, v.object_key
          FROM templates t
          JOIN template_versions v ON v.template_id = t.id AND v.version = t.version
          WHERE t.id = ${id}`,
      });
      const writtenKey = SqlSchema.findOneOption({
        Request: VersionKey,
        Result: Schema.Struct({ objectKey: Schema.String }),
        execute: ({ id, version }) => sql`
          SELECT object_key FROM template_versions
          WHERE template_id = ${id} AND version = ${version}`,
      });

      const find = (id: string) =>
        templates
          .findById(TemplateId.make(id))
          .pipe(Effect.catchNoSuchElement, storage("read the template"));

      const list = all(undefined).pipe(
        Effect.map((rows) => rows.map(toEntry)),
        storage("list the templates"),
        Effect.withSpan("Library.list"),
      );

      const managed = managedRows(undefined).pipe(
        storage("list the templates"),
        Effect.withSpan("Library.managed"),
      );

      const versions = Effect.fn("Library.versions")(function* (id: string) {
        const template = yield* find(id);
        if (Option.isNone(template)) return yield* notFound();
        return yield* versionRows(id).pipe(storage("read the version history"));
      });

      const download = Effect.fn("Library.download")(function* (id: string) {
        const current = yield* currentObject(id).pipe(storage("read the template"));
        if (Option.isNone(current)) return yield* notFound();
        const { version, objectKey: key } = current.value;
        const content = yield* blobs.get(key).pipe(storage("read the template"));
        if (Option.isNone(content)) return yield* notFound();
        return { version, content: content.value };
      });

      const advance = Effect.fnUntraced(function* (
        current: Template,
        next: NextVersion,
        by: UserId,
      ) {
        const version = current.version + 1;
        const now = DateTime.toEpochMillis(yield* DateTime.now);
        const { objectKey: key, restoredFrom, name, fieldCount, preview, size } = next;
        const fields = {
          name,
          version,
          fieldCount,
          preview,
          size,
          publishedBy: by,
          updatedAt: now,
        };
        yield* atomic
          .run([
            sql`INSERT INTO template_versions (
                template_id, version, name, field_count, preview, size, object_key,
                published_by, published_at, restored_from
              )
              SELECT ${current.id}, ${version}, ${name}, ${fieldCount}, ${preview}, ${size},
                ${key}, ${by}, ${now}, ${restoredFrom}
              WHERE EXISTS (
                SELECT 1 FROM templates WHERE id = ${current.id} AND version = ${current.version}
              )`,
            sql`UPDATE templates SET ${sql.update(fields)}
              WHERE id = ${current.id} AND version = ${current.version}`,
          ])
          .pipe(storage("publish the template"));
        const written = yield* writtenKey({ id: current.id, version }).pipe(
          storage("publish the template"),
        );
        return Option.isSome(written) && written.value.objectKey === key;
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
            sql`INSERT INTO templates ${sql.insert({ id, ...shared, createdAt: now, updatedAt: now })}`,
            sql`INSERT INTO template_versions ${sql.insert({
              templateId: id,
              ...shared,
              objectKey: key,
              publishedAt: now,
            })}`,
          ])
          .pipe(storage("publish the template"));
        return { id, version: 1 };
      });

      const update = Effect.fn("Library.update")(function* (
        id: string,
        upload: Upload,
        by: UserId,
      ) {
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
        const target = yield* storedVersion({ id, version }).pipe(
          storage("read the version history"),
        );
        if (Option.isNone(target)) return yield* notFound();
        const written = yield* advance(
          current.value,
          { ...target.value, restoredFrom: version },
          by,
        );
        if (!written) return yield* conflict();
        return { id: current.value.id, version: current.value.version + 1 };
      });

      const remove = Effect.fn("Library.remove")(function* (id: string) {
        const template = yield* find(id);
        if (Option.isNone(template)) return yield* notFound();
        yield* atomic
          .run([
            sql`DELETE FROM template_versions WHERE template_id = ${id}`,
            sql`DELETE FROM templates WHERE id = ${id}`,
          ])
          .pipe(storage("remove the template"));
        yield* blobs.removePrefix(`templates/${id}/`).pipe(storage("remove the template"));
      });

      return Library.of({ list, managed, versions, download, create, update, restore, remove });
    }),
  );
}
