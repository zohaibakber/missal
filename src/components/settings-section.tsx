import type { ReactNode } from "react";

export function SettingsSection({
  title,
  description,
  children,
}: {
  title: string;
  description?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="grid gap-x-12 gap-y-4 border-t py-8 first:border-t-0 first:pt-0 @3xl:grid-cols-[14rem_minmax(0,1fr)]">
      <div className="flex flex-col gap-1">
        <h2 className="text-sm font-medium">{title}</h2>
        {description ? (
          <p className="text-sm text-pretty text-muted-foreground">{description}</p>
        ) : null}
      </div>
      <div className="min-w-0">{children}</div>
    </section>
  );
}
