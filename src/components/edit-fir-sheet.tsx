import { EditFirForm } from "#/components/create-fir-form";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "#/components/ui/sheet";
import { toast } from "#/components/ui/toast";
import type { FirRecord } from "#/lib/fir";

/** The one place FIR details are edited, from both the FIR list and the FIR workspace. */
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
      <SheetContent className="data-[side=right]:sm:max-w-2xl">
        <SheetHeader>
          <SheetTitle>Edit FIR {fir.fir_no}</SheetTitle>
          <SheetDescription>Case details are used by every document in this FIR.</SheetDescription>
        </SheetHeader>
        <EditFirForm
          className="min-h-0 flex-1"
          fir={fir}
          onSuccess={() => {
            onOpenChange(false);
            toast.add({ title: "FIR details saved", type: "success" });
          }}
        />
      </SheetContent>
    </Sheet>
  );
}
