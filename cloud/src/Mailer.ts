import * as Context from "effect/Context";
import * as SecureRandom from "effect/crypto/SecureRandom";
import * as Data from "effect/Data";
import * as Effect from "effect/Effect";
import * as HttpClient from "effect/http/HttpClient";
import * as HttpClientRequest from "effect/http/HttpClientRequest";
import * as Layer from "effect/Layer";
import type * as Redacted from "effect/Redacted";
import * as Ref from "effect/Ref";
import * as Schedule from "effect/Schedule";

const RESEND_URL = "https://api.resend.com/emails";

export interface MailMessage {
  readonly to: string;
  readonly subject: string;
  readonly text: string;
  readonly html?: string;
}

export interface ResendOptions {
  readonly apiKey: Redacted.Redacted<string>;
  readonly from: string;
}

export class MailFailure extends Data.TaggedError("MailFailure")<{ readonly reason: string }> {}

export class Outbox extends Context.Service<Outbox, Ref.Ref<ReadonlyArray<MailMessage>>>()(
  "Outbox",
) {}

export class Mailer extends Context.Service<
  Mailer,
  { readonly send: (message: MailMessage) => Effect.Effect<void, MailFailure> }
>()("Mailer") {
  static readonly layerResend = (options: ResendOptions) =>
    Layer.effect(
      Mailer,
      Effect.gen(function* () {
        const random = yield* SecureRandom.SecureRandom;
        const client = (yield* HttpClient.HttpClient).pipe(
          HttpClient.timeout("10 seconds"),
          HttpClient.retryTransient({ schedule: Schedule.exponential("250 millis"), times: 3 }),
          HttpClient.filterStatusOk,
        );
        return Mailer.of({
          send: Effect.fn("Mailer.send")(function* (message: MailMessage) {
            const request = HttpClientRequest.post(RESEND_URL).pipe(
              HttpClientRequest.bearerToken(options.apiKey),
              HttpClientRequest.setHeader("idempotency-key", yield* random.uuidV4()),
              HttpClientRequest.bodyJsonUnsafe({
                from: options.from,
                to: [message.to],
                subject: message.subject,
                text: message.text,
                ...(message.html === undefined ? {} : { html: message.html }),
              }),
            );
            yield* client.execute(request).pipe(
              Effect.mapError(
                (error) =>
                  new MailFailure({
                    reason:
                      error.response === undefined
                        ? error.reason._tag
                        : `Resend answered ${error.response.status}`,
                  }),
              ),
            );
          }),
        });
      }),
    );

  static readonly layerLog = Layer.succeed(
    Mailer,
    Mailer.of({
      send: (message) =>
        Effect.logWarning("email not sent, RESEND_API_KEY is not set", {
          to: message.to,
          subject: message.subject,
          text: message.text,
        }),
    }),
  );

  static readonly layerMemory = Layer.effect(
    Mailer,
    Effect.map(Outbox, (outbox) =>
      Mailer.of({
        send: (message) => Ref.update(outbox, (sent) => [...sent, message]),
      }),
    ),
  ).pipe(Layer.provideMerge(Layer.effect(Outbox, Ref.make<ReadonlyArray<MailMessage>>([]))));
}
