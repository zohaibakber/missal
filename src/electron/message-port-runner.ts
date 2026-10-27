import { Cause, Deferred, Effect, Exit, Fiber, Layer, Scope } from "effect";
import { WorkerError, WorkerReceiveError } from "effect/workers/WorkerError";
import { type PlatformMessage, WorkerRunnerPlatform } from "effect/workers/WorkerRunner";
import type { MessagePort, Transferable } from "node:worker_threads";

export const layer = (port: MessagePort) =>
  Layer.succeed(WorkerRunnerPlatform)({
    start: <O, I>() => {
      const sendUnsafe = (_clientId: number, message: O, transfers?: ReadonlyArray<unknown>) =>
        port.postMessage([1, message], transfers as Transferable[]);

      return Effect.succeed({
        sendUnsafe,
        send: (clientId: number, message: O, transfers?: ReadonlyArray<unknown>) =>
          Effect.sync(() => sendUnsafe(clientId, message, transfers)),
        run: <A, E, R>(handler: (clientId: number, message: I) => Effect.Effect<A, E, R> | void) =>
          Effect.scopedWith((scope) =>
            Effect.gen(function* () {
              const closed = Deferred.makeUnsafe<void, WorkerError>();
              const runFork = Effect.runForkWith(yield* Effect.context<R>());
              const track = Fiber.runIn(scope);
              const onExit = (exit: Exit.Exit<A, E>) => {
                if (Exit.isFailure(exit) && !Cause.hasInterruptsOnly(exit.cause)) {
                  runFork(Effect.logError("unhandled error in worker", exit.cause));
                }
              };
              const onMessage = (message: PlatformMessage<I>) => {
                if (message[0] === 1) return Deferred.doneUnsafe(closed, Exit.void);
                const result = handler(0, message[1]);
                if (Effect.isEffect(result)) track(runFork(result)).addObserver(onExit);
              };
              const onMessageError = (cause: unknown) =>
                Deferred.doneUnsafe(
                  closed,
                  Exit.fail(
                    new WorkerError({
                      reason: new WorkerReceiveError({ message: "received messageerror", cause }),
                    }),
                  ),
                );
              const onClose = () => Deferred.doneUnsafe(closed, Exit.void);

              port.on("message", onMessage).on("messageerror", onMessageError).on("close", onClose);
              yield* Scope.addFinalizer(
                scope,
                Effect.sync(() => {
                  port.off("message", onMessage).off("messageerror", onMessageError);
                  port.off("close", onClose).close();
                }),
              );
              port.postMessage([0]);
              return yield* Deferred.await(closed);
            }),
          ),
      });
    },
  });
