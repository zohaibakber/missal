import { DateTime, Effect, Schema } from "effect";
import { IsoDateTimeString } from "#/lib/schema";

export const nowIso = DateTime.now.pipe(
  Effect.map((instant) => Schema.decodeUnknownSync(IsoDateTimeString)(DateTime.formatIso(instant))),
);
