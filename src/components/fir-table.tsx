import { flightsData } from "#/lib/fir-data";
import { DataTable } from "./data-table";
import { firColumns } from "./fir-columns";

export function FirTable() {
  return <DataTable columns={firColumns} data={flightsData} />;
}
