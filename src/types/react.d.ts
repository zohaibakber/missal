import "react";

declare module "react" {
  interface CSSProperties {
    "--column-width"?: string;
    "--page-width"?: string;
    "--page-height"?: string;
    "--page-top"?: string;
    "--page-right"?: string;
    "--page-bottom"?: string;
    "--page-left"?: string;
  }
}
