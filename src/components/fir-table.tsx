import type { DataTableToolbarConfig } from "#/components/data-table-toolbar";
import { DataTable } from "./data-table";
import { firColumns } from "./fir-columns";
import { flightsData, type Flight } from "#/lib/fir-data";

const flightTableToolbar: DataTableToolbarConfig<Flight> = {
  search: {
    label: "Search",
    placeholder: "Search",
    searchableColumnIds: ["flightCode", "destination", "status", "terminal", "gate"],
  },
  filters: [
    {
      columnId: "status",
      label: "Status",
      placeholder: "All statuses",
      options: [
        { label: "On Time", value: "On Time" },
        { label: "Delayed", value: "Delayed" },
        { label: "Boarding", value: "Boarding" },
        { label: "Cancelled", value: "Cancelled" },
      ],
    },
    {
      columnId: "terminal",
      label: "Terminal",
      placeholder: "All terminals",
      options: [
        { label: "Terminal 1", value: "1" },
        { label: "Terminal 2", value: "2" },
        { label: "Terminal 3", value: "3" },
      ],
    },
  ],
};

export function FirTable() {
  return <DataTable columns={firColumns} data={flightsData} toolbar={flightTableToolbar} />;
}
