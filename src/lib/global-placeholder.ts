import { Schema } from "effect";
import { PlaceholderId } from "#/lib/ids";
import { PlaceholderName } from "#/lib/placeholder";

export class GlobalPlaceholder extends Schema.Class<GlobalPlaceholder>("GlobalPlaceholder")({
  id: PlaceholderId,
  label: PlaceholderName,
  value: Schema.String,
}) {}

export const GlobalPlaceholderDraft = Schema.Union([
  Schema.TaggedStruct("Existing", {
    id: PlaceholderId,
    label: PlaceholderName,
    value: Schema.String,
  }),
  Schema.TaggedStruct("New", { label: PlaceholderName, value: Schema.String }),
]);

export class SaveGlobalPlaceholdersInput extends Schema.Class<SaveGlobalPlaceholdersInput>(
  "SaveGlobalPlaceholdersInput",
)({
  entries: Schema.Array(GlobalPlaceholderDraft),
}) {}
