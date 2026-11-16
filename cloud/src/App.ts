import * as Digest from "effect/crypto/Digest";
import * as Kdf from "effect/crypto/Kdf";
import * as Mac from "effect/crypto/Mac";
import * as SecureRandom from "effect/crypto/SecureRandom";
import * as HttpApiBuilder from "effect/http-api/HttpApiBuilder";
import * as Layer from "effect/Layer";
import { Accounts } from "./Accounts.ts";
import { MissalApi } from "./Api.ts";
import { AuthorizationLive } from "./Auth.ts";
import { HandlersLive } from "./Handlers.ts";
import { Library } from "./Library.ts";
import { Passwords } from "./Passwords.ts";
import { Tokens } from "./Tokens.ts";

export const CryptoLive = Layer.mergeAll(
  Kdf.layerWebCrypto,
  Digest.layerWebCrypto,
  Mac.layerWebCrypto,
).pipe(Layer.provideMerge(SecureRandom.layerWebCrypto));

export const ServicesLive = Layer.mergeAll(Accounts.layer, Library.layer).pipe(
  Layer.provideMerge(Layer.mergeAll(Passwords.layer, Tokens.layer)),
);

export const ApiLive = HttpApiBuilder.layer(MissalApi, { openapiPath: "/openapi.json" }).pipe(
  Layer.provide(HandlersLive),
  Layer.provide(AuthorizationLive),
  Layer.provide(ServicesLive),
);
