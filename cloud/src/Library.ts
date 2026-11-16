import type { ReadWriteBucketClient } from "@alchemy.run/cloudflare/R2/ReadWriteBucket";
import type { RuntimeContext } from "alchemy/RuntimeContext";
import * as Context from "effect/Context";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import * as Option from "effect/Option";
import * as Schema from "effect/Schema";
import type * as SqlClient from "effect/sql/SqlClient";
import {
  type Entry,
  PublishConflict,
  StorageFailure,
  TemplateNotFound,
  type Upload,
} from "./Api.ts";

const Row = Schema.Struct({
  id: Schema.String,
  name: Schema.String,
  version: Schema.Int,
  field_count: Schema.Int,
  preview: Schema.String,
  updated_at: Schema.Number,
});

const VersionRow = Schema.Struct({ version: Schema.Int });

const decodeRows = Schema.decodeUnknownEffect(Schema.Array(Row));
const decodeVersion = Schema.decodeUnknownEffect(Schema.Array(VersionRow));

const objectKey = (id: string, version: number) => `templates/${id}/${version}.msb`;

const notFound = () => new TemplateNotFound({ error: "not found" });

const storage =
  (operation: string) =>
  <A, E, R>(effect: Effect.Effect<A, E, R>): Effect.Effect<A, StorageFailure, R> =>
    effect.pipe(
      Effect.tapError((cause) => Effect.logError(`library ${operation} failed`, cause)),
      Effect.mapError(() => new StorageFailure({ error: `could not ${operation} the template` })),
    );

export interface Stored {
  readonly version: number;
  readonly content: Uint8Array;
}

export interface Published {
  readonly id: string;
  readonly version: number;
}

export class Library extends Context.Service<
  Library,
  {
    readonly list: Effect.Effect<ReadonlyArray<Entry>, StorageFailure, RuntimeContext>;
    readonly download: (
      id: string,
    ) => Effect.Effect<Stored, TemplateNotFound | StorageFailure, RuntimeContext>;
    readonly create: (
      upload: Upload,
      by: string,
    ) => Effect.Effect<Published, StorageFailure, RuntimeContext>;
    readonly update: (
      id: string,
      upload: Upload,
      by: string,
    ) => Effect.Effect<
      Published,
      TemplateNotFound | PublishConflict | StorageFailure,
      RuntimeContext
    >;
    readonly remove: (
      id: string,
    ) => Effect.Effect<void, TemplateNotFound | StorageFailure, RuntimeContext>;
  }
>()("missal/Library") {}

export const LibraryLive = (sql: SqlClient.SqlClient, bucket: ReadWriteBucketClient) =>
  Layer.sync(Library, () => {
    const currentVersion = (id: string) =>
      sql`SELECT version FROM templates WHERE id = ${id}`.pipe(
        Effect.flatMap(decodeVersion),
        Effect.map((rows) => Option.fromNullishOr(rows[0]?.version)),
      );

    const list = sql`SELECT id, name, version, field_count, preview, updated_at FROM templates ORDER BY name`.pipe(
      Effect.flatMap(decodeRows),
      Effect.map((rows) =>
        rows.map(
          (row): Entry => ({
            id: row.id,
            name: row.name,
            version: row.version,
            fieldCount: row.field_count,
            preview: row.preview,
            updatedAt: row.updated_at,
          }),
        ),
      ),
      storage("list"),
      Effect.withSpan("library.list"),
    );

    const download = Effect.fn("library.download")(function* (id: string) {
      const version = yield* currentVersion(id).pipe(storage("read"));
      if (Option.isNone(version)) return yield* notFound();
      const object = yield* bucket.get(objectKey(id, version.value)).pipe(storage("read"));
      if (object === null) return yield* notFound();
      const content = yield* object.bytes().pipe(storage("read"));
      return { version: version.value, content };
    });

    const create = Effect.fn("library.create")(function* (upload: Upload, by: string) {
      const id = crypto.randomUUID();
      const now = Date.now();
      yield* bucket.put(objectKey(id, 1), upload.content).pipe(storage("store"));
      yield* sql`INSERT INTO templates ${sql.insert({
        id,
        name: upload.name,
        version: 1,
        field_count: upload.fieldCount,
        preview: upload.preview,
        size: upload.content.length,
        published_by: by,
        created_at: now,
        updated_at: now,
      })}`.pipe(storage("publish"));
      return { id, version: 1 };
    });

    const update = Effect.fn("library.update")(function* (
      id: string,
      upload: Upload,
      by: string,
    ) {
      const current = yield* currentVersion(id).pipe(storage("read"));
      if (Option.isNone(current)) return yield* notFound();
      const version = current.value + 1;
      yield* bucket.put(objectKey(id, version), upload.content).pipe(storage("store"));
      const changed = yield* sql`UPDATE templates SET ${sql.update({
        name: upload.name,
        version,
        field_count: upload.fieldCount,
        preview: upload.preview,
        size: upload.content.length,
        published_by: by,
        updated_at: Date.now(),
      })} WHERE id = ${id} AND version = ${current.value} RETURNING version`.pipe(
        Effect.flatMap(decodeVersion),
        storage("publish"),
      );
      if (changed.length === 0) {
        yield* bucket.delete(objectKey(id, version)).pipe(Effect.ignore);
        return yield* new PublishConflict({
          error: "someone else published this template at the same time; try again",
        });
      }
      yield* bucket.delete(objectKey(id, current.value)).pipe(Effect.ignore);
      return { id, version };
    });

    const remove = Effect.fn("library.remove")(function* (id: string) {
      const removed = yield* sql`DELETE FROM templates WHERE id = ${id} RETURNING version`.pipe(
        Effect.flatMap(decodeVersion),
        storage("remove"),
      );
      if (removed.length === 0) return yield* notFound();
      const listing = yield* bucket.list({ prefix: `templates/${id}/` }).pipe(storage("remove"));
      if (listing.objects.length > 0) {
        yield* bucket
          .delete(listing.objects.map((object) => object.key))
          .pipe(storage("remove"));
      }
    });

    return Library.of({ list, download, create, update, remove });
  });
