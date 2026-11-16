import type { ReactNode } from "react";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

/** A labelled select over a list of options. */
export function ChoiceField({
  id,
  label,
  value,
  options,
  onChange,
  description,
  action,
}: {
  id: string;
  label: string;
  value: string;
  /** `extra` shows after the label in the list only, not in the closed select. */
  options: { value: string; label: string; extra?: ReactNode }[];
  onChange: (value: string) => void;
  description?: ReactNode;
  /** A button beside the select, acting on the chosen option. */
  action?: ReactNode;
}) {
  return (
    <Field>
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <div className="flex gap-2">
        <Select
          name={id}
          value={value}
          onValueChange={(next) => next !== null && onChange(next)}
          items={options}
        >
          <SelectTrigger id={id} className="w-full">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectGroup>
              {options.map((option) => (
                <SelectItem key={option.value} value={option.value}>
                  {option.label}
                  {option.extra}
                </SelectItem>
              ))}
            </SelectGroup>
          </SelectContent>
        </Select>
        {action}
      </div>
      {description && <FieldDescription>{description}</FieldDescription>}
    </Field>
  );
}
