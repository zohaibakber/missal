import { lazy, Suspense } from "react";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "#/components/ui/sheet";
import { toast } from "#/components/ui/toast";
import type { FirRecord } from "#/lib/fir";

const EditFirForm = lazy(() =>
  import("#/components/create-fir-form").then((module) => ({ default: module.EditFirForm })),
);

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
        <Suspense fallback={null}>
          <EditFirForm
            className="min-h-0 flex-1"
            fir={fir}
            onSuccess={() => {
              onOpenChange(false);
              toast.add({ title: "FIR details saved", type: "success" });
            }}
          />
        </Suspense>
      </SheetContent>
    </Sheet>
  );
}
