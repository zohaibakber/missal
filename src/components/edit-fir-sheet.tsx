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
import { toast } from "#/components/ui/toast";
import type { FirRecord } from "#/lib/fir";

export function EditFirSheet({
  fir,
  open,
  onOpenChange,
}: {
  fir: FirRecord;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent size="lg">
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
      </SheetContent>
    </Sheet>
  );
}
