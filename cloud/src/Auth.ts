import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import * as Redacted from "effect/Redacted";
import * as Schema from "effect/Schema";
import { createRemoteJWKSet, jwtVerify } from "jose";
import { Authentication, NotPublisher, PublishersOnly, Unauthorized, Viewer } from "./Api.ts";

const PUBLISH_ROLE = "admin";
const PUBLISH_PERMISSION = "templates:publish";

const Claims = Schema.Struct({
  sub: Schema.NonEmptyString,
  role: Schema.optional(Schema.String),
  roles: Schema.optional(Schema.Array(Schema.String)),
  permissions: Schema.optional(Schema.Array(Schema.String)),
});

const decodeClaims = Schema.decodeUnknownEffect(Claims);

const signInRequired = () => new Unauthorized({ error: "sign in to use the template library" });

export interface WorkOSOptions {
  readonly baseUrl: string;
  readonly clientId: string;
  readonly publishers: ReadonlySet<string>;
}

export const AuthenticationLive = (options: WorkOSOptions) =>
  Layer.sync(Authentication, () => {
    const keys = createRemoteJWKSet(
      new URL(`sso/jwks/${options.clientId}`, `${options.baseUrl.replace(/\/+$/, "")}/`),
    );
    const viewer = Effect.fn("Authentication.viewer")(function* (token: string) {
      const { payload } = yield* Effect.tryPromise({
        try: () => jwtVerify(token, keys, { algorithms: ["RS256"] }),
        catch: signInRequired,
      });
      const claims = yield* decodeClaims(payload).pipe(Effect.mapError(signInRequired));
      const roles = [claims.role, ...(claims.roles ?? [])];
      return Viewer.of({
        id: claims.sub,
        publisher:
          options.publishers.has(claims.sub) ||
          roles.includes(PUBLISH_ROLE) ||
          (claims.permissions ?? []).includes(PUBLISH_PERMISSION),
      });
    });
    return Authentication.of({
      bearer: (httpEffect, { credential }) =>
        viewer(Redacted.value(credential)).pipe(
          Effect.flatMap((who) => Effect.provideService(httpEffect, Viewer, who)),
        ),
    });
  });

export const PublishersOnlyLive = Layer.succeed(
  PublishersOnly,
  PublishersOnly.of((httpEffect) =>
    Effect.gen(function* () {
      const who = yield* Viewer;
      if (!who.publisher) {
        return yield* new NotPublisher({ error: "this account cannot publish templates" });
      }
      return yield* httpEffect;
    }),
  ),
);

export const parsePublishers = (value: string): ReadonlySet<string> =>
  new Set(
    value
      .split(",")
      .map((id) => id.trim())
      .filter((id) => id.length > 0),
  );
