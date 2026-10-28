import { useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/reactivity";
import {
  FirForm,
  FirFormFields,
  FirFormReset,
  FirFormStatus,
  FirFormSubmit,
} from "#/components/fir-form";
import { Sheet, SheetContent, SheetFooter, SheetHeader, SheetTitle } from "#/components/ui/sheet";
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
              <SheetTitle>Edit FIR {fir.fir_no}</SheetTitle>
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
              <SheetTitle>Edit FIR</SheetTitle>
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

export function EditFirSheetById({ firId, ...props }: EditFirSheetProps & { firId: FirId }) {
  const result = useAtomValue(atoms.firByIdAtom(firId));
  return <EditFirSheet fir={AsyncResult.isSuccess(result) ? result.value : null} {...props} />;
}
