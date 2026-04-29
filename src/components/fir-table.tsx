import type { SortingState } from "@tanstack/react-table";
import { useLiveQuery } from "@tanstack/react-db";
import type { DataTableToolbarConfig } from "#/components/data-table-toolbar";
import { DataTable } from "./data-table";
import { firColumns } from "./fir-columns";
import { firCollection } from "#/db-collections";
import type { FirRecord } from "#/lib/fir";
import { FIR_STATUS_OPTIONS } from "#/lib/fir";

const firTableToolbar: DataTableToolbarConfig<FirRecord> = {
  search: {
    label: "Search FIR records",
    placeholder: "",
    searchableColumnIds: [
      "fir_no",
      "date",
      "incident_date",
      "offence",
      "accused",
      "witness",
      "NIC",
      "mobile",
      "status",
    ],
  },
  filters: [
    {
      columnId: "status",
      label: "Status",
      placeholder: "All statuses",
      options: FIR_STATUS_OPTIONS.map((status) => ({
        label: status,
        value: status,
      })),
    },
  ],
};

const initialSorting: SortingState = [
  {
    desc: true,
    id: "fir_no",
  },
];

export function FirTable() {
  const { data } = useLiveQuery(firCollection);

  return (
    <DataTable
      columns={firColumns}
      data={data}
      initialSorting={initialSorting}
      tableDir="rtl"
      tableLang="ur"
      toolbar={firTableToolbar}
    />
  );
}
