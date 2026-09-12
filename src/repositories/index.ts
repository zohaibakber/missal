import { FirDocumentRepository } from "#/repositories/fir-document-repository";
import { FirPlaceholderValueRepository } from "#/repositories/fir-placeholder-value-repository";
import { FirRepository } from "#/repositories/fir-repository";
import { PlaceholderRepository } from "#/repositories/placeholder-repository";
import { SettingsRepository } from "#/repositories/settings-repository";
import { TemplateRepository } from "#/repositories/template-repository";

export type AppRepositories =
  | PlaceholderRepository
  | TemplateRepository
  | FirRepository
  | FirDocumentRepository
  | FirPlaceholderValueRepository
  | SettingsRepository;

export { FirDocumentRepository } from "#/repositories/fir-document-repository";
export { FirPlaceholderValueRepository } from "#/repositories/fir-placeholder-value-repository";
export { FirRepository } from "#/repositories/fir-repository";
export { PlaceholderRepository } from "#/repositories/placeholder-repository";
export { SettingsRepository } from "#/repositories/settings-repository";
export { TemplateRepository } from "#/repositories/template-repository";
