import { FirPlaceholderValueRepository } from "#/repositories/fir-placeholder-value-repository";
import { FirRepository } from "#/repositories/fir-repository";
import { PlaceholderRepository } from "#/repositories/placeholder-repository";
import { SettingsRepository } from "#/repositories/settings-repository";
import { TemplateRepository } from "#/repositories/template-repository";

export type AppRepositories =
  | PlaceholderRepository
  | TemplateRepository
  | FirRepository
  | FirPlaceholderValueRepository
  | SettingsRepository;

export { FirPlaceholderValueRepository } from "#/repositories/fir-placeholder-value-repository";
export { FirRepository } from "#/repositories/fir-repository";
export { PlaceholderRepository } from "#/repositories/placeholder-repository";
export { SettingsRepository } from "#/repositories/settings-repository";
export { TemplateRepository } from "#/repositories/template-repository";
