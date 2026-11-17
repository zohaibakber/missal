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
  PublishConflict,
  type StorageFailure,
  TemplateNotFound,
  type Upload,
} from "./Api.ts";
import { Blobs } from "./Blobs.ts";
import { storage } from "./Failures.ts";
import { Template, TemplateId, type UserId } from "./Models.ts";

const objectKey = (id: string, version: number) => `templates/${id}/${version}.msb`;

const notFound = () => new TemplateNotFound({ error: "not found" });

const toEntry = (template: Template): Entry => template;

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
    readonly download: (id: string) => Effectful<Stored, TemplateNotFound | StorageFailure>;
    readonly create: (upload: Upload, by: UserId) => Effectful<Published, StorageFailure>;
    readonly update: (
      id: string,
      upload: Upload,
      by: UserId,
    ) => Effectful<Published, TemplateNotFound | PublishConflict | StorageFailure>;
    readonly remove: (id: string) => Effectful<void, TemplateNotFound | StorageFailure>;
  }
>()("Library") {
  static readonly layer = Layer.effect(
    Library,
    Effect.gen(function* () {
      const sql = yield* SqlClient.SqlClient;
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

      const find = (id: string) =>
        templates.findById(TemplateId.make(id)).pipe(
          Effect.map(Option.some),
          Effect.catchTag("NoSuchElementError", () => Effect.succeedNone),
          storage("read the template"),
        );

      const list = all(undefined).pipe(
        Effect.map((rows) => rows.map(toEntry)),
        storage("list the templates"),
        Effect.withSpan("Library.list"),
      );

      const download = Effect.fn("Library.download")(function* (id: string) {
        const template = yield* find(id);
        if (Option.isNone(template)) return yield* notFound();
        const { version } = template.value;
        const content = yield* blobs.get(objectKey(id, version)).pipe(storage("read the template"));
        if (Option.isNone(content)) return yield* notFound();
        return { version, content: content.value };
      });

      const create = Effect.fn("Library.create")(function* (upload: Upload, by: UserId) {
        const id = TemplateId.make(yield* random.uuidV4());
        yield* blobs.put(objectKey(id, 1), upload.content).pipe(storage("store the template"));
        const created = yield* templates
          .insert(
            Template.insert.make({
              id,
              name: upload.name,
              version: 1,
              fieldCount: upload.fieldCount,
              preview: upload.preview,
              size: upload.content.length,
              publishedBy: by,
            }),
          )
          .pipe(storage("publish the template"));
        return { id: created.id, version: created.version };
      });

      const update = Effect.fn("Library.update")(function* (
        id: string,
        upload: Upload,
        by: UserId,
      ) {
        const current = yield* find(id);
        if (Option.isNone(current)) return yield* notFound();
        const version = current.value.version + 1;
        yield* blobs
          .put(objectKey(id, version), upload.content)
          .pipe(storage("store the template"));
        const fields = {
          name: upload.name,
          version,
          fieldCount: upload.fieldCount,
          preview: upload.preview,
          size: upload.content.length,
          publishedBy: by,
          updatedAt: DateTime.toEpochMillis(yield* DateTime.now),
        };
        const updated =
          yield* sql`UPDATE templates SET ${sql.update(fields)} WHERE id = ${id} AND version = ${current.value.version} RETURNING id`.pipe(
            storage("publish the template"),
          );
        if (updated.length === 0) {
          yield* blobs.remove(objectKey(id, version)).pipe(Effect.ignore);
          return yield* new PublishConflict({
            error: "someone else published this template at the same time; try again",
          });
        }
        yield* blobs.remove(objectKey(id, current.value.version)).pipe(Effect.ignore);
        return { id: current.value.id, version };
      });

      const remove = Effect.fn("Library.remove")(function* (id: string) {
        const template = yield* find(id);
        if (Option.isNone(template)) return yield* notFound();
        yield* templates.delete(template.value.id).pipe(storage("remove the template"));
        yield* blobs.removePrefix(`templates/${id}/`).pipe(storage("remove the template"));
      });

      return Library.of({ list, download, create, update, remove });
    }),
  );
}
