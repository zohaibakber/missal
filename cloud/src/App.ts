import * as Digest from "effect/crypto/Digest";
import * as Kdf from "effect/crypto/Kdf";
import * as Mac from "effect/crypto/Mac";
import * as SecureRandom from "effect/crypto/SecureRandom";
import * as HttpApiBuilder from "effect/http-api/HttpApiBuilder";
import * as Layer from "effect/Layer";
import { Accounts } from "./Accounts.ts";
import { Api } from "./Api.ts";
import { AuthorizationLive } from "./Auth.ts";
import { HandlersLive } from "./Handlers.ts";
import { Codes } from "./Codes.ts";
import { Library } from "./Library.ts";
import { PasswordLive } from "./Passwords.ts";
import { People } from "./People.ts";
import { Sessions } from "./Sessions.ts";
import { SignInLockout } from "./Throttle.ts";
import { Tokens } from "./Tokens.ts";
import { Users } from "./Users.ts";

export const CryptoLive = Layer.mergeAll(
  Kdf.layerWebCrypto,
  Digest.layerWebCrypto,
  Mac.layerWebCrypto,
).pipe(Layer.provideMerge(SecureRandom.layerWebCrypto));

export const ServicesLive = Layer.mergeAll(Accounts.layer, People.layer, Library.layer).pipe(
  Layer.provideMerge(Layer.mergeAll(Sessions.layer, Codes.layer, SignInLockout.layer)),
  Layer.provideMerge(Layer.mergeAll(Users.layer, Tokens.layer, PasswordLive)),
);

export const ApiLive = HttpApiBuilder.layer(Api, { openapiPath: "/openapi.json" }).pipe(
  Layer.provide(HandlersLive),
  Layer.provide(AuthorizationLive),
  Layer.provide(ServicesLive),
);
