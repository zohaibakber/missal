import { useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import {
  FirForm,
  FirFormFields,
  FirFormReset,
  FirFormStatus,
  FirFormSubmit,
} from "#/components/fir-form";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from "#/components/ui/sheet";
import { Spinner } from "#/components/ui/spinner";
import { toast } from "#/components/ui/toast";
import type { FirId, FirRecord } from "#/lib/fir";
import { atoms } from "#/state/atoms";

type EditFirSheetProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export function EditFirSheet({
  fir,
  open,
  onOpenChange,
}: EditFirSheetProps & {
  /** Null while the record is still loading. */
  fir: FirRecord | null;
}) {
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent size="lg">
        {fir ? (
          <FirForm
            fir={fir}
            className="flex min-h-0 flex-1 flex-col"
            onSuccess={() => {
              onOpenChange(false);
              toast.add({ title: "FIR details saved", type: "success" });
            }}
          >
            <SheetHeader>
              <SheetTitle>FIR {fir.fir_no}</SheetTitle>
              <SheetDescription>Every document in this FIR uses these details.</SheetDescription>
            </SheetHeader>
            <div className="min-h-0 flex-1 overflow-y-auto p-6">
              <FirFormFields />
            </div>
            <SheetFooter>
              <FirFormStatus />
              <FirFormReset />
              <FirFormSubmit>Save changes</FirFormSubmit>
            </SheetFooter>
          </FirForm>
        ) : (
          <>
            <SheetHeader>
              <SheetTitle>FIR details</SheetTitle>
              <SheetDescription>Loading…</SheetDescription>
            </SheetHeader>
            <div className="flex flex-1 items-center justify-center" aria-busy="true">
              <Spinner />
            </div>
          </>
        )}
      </SheetContent>
    </Sheet>
  );
}

/** Edits a FIR from the list, whose rows only carry a summary, by loading the full record. */
export function EditFirSheetById({ firId, ...props }: EditFirSheetProps & { firId: FirId }) {
  const result = useAtomValue(atoms.firByIdAtom(firId));
  return <EditFirSheet fir={AsyncResult.isSuccess(result) ? result.value : null} {...props} />;
}
