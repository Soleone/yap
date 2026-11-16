import type { ComponentProps } from "react";
import { cn } from "cn";
import { DialogContent } from "@/components/ui/dialog";

/** Wide enough for settings fields. Its top stays put, so switching between tabs of different heights doesn't jump. */
export function SettingsDialogContent({
  className,
  ...props
}: ComponentProps<typeof DialogContent>) {
  return (
    <DialogContent
      className={cn(
        "top-8 max-h-[calc(100svh-4rem)] translate-y-0 content-start overflow-y-auto sm:max-w-xl",
        className,
      )}
      {...props}
    />
  );
}
