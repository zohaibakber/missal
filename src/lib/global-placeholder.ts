import { Schema } from "effect";
import { PlaceholderId } from "#/lib/ids";
import { PlaceholderKey } from "#/lib/placeholder";
import { NonEmptyTrimmedString } from "#/lib/schema";

export class GlobalPlaceholder extends Schema.Class<GlobalPlaceholder>("GlobalPlaceholder")({
  id: PlaceholderId,
  key: PlaceholderKey,
  label: NonEmptyTrimmedString,
  value: Schema.String,
}) {}

export const GlobalPlaceholderDraft = Schema.Union([
  Schema.TaggedStruct("Existing", {
    id: PlaceholderId,
    label: NonEmptyTrimmedString,
    value: Schema.String,
  }),
  Schema.TaggedStruct("New", { label: NonEmptyTrimmedString, value: Schema.String }),
]);

export class SaveGlobalPlaceholdersInput extends Schema.Class<SaveGlobalPlaceholdersInput>(
  "SaveGlobalPlaceholdersInput",
)({
  entries: Schema.Array(GlobalPlaceholderDraft),
}) {}
