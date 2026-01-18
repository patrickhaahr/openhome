import type { Component } from "solid-js";
import {
  Show,
  createSignal,
  splitProps,
  type ComponentProps,
  type ParentComponent,
} from "solid-js";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";

interface DialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

const Dialog: Component<DialogProps & { children: unknown }> = (props) => {
  return (
    <Show when={props.open}>
      {props.children}
    </Show>
  );
};

const DialogTrigger: ParentComponent<ComponentProps<"button">> = (props) => {
  const [local, others] = splitProps(props, ["children"]);
  return (
    <button type="button" {...others}>
      {local.children}
    </button>
  );
};

const DialogPortal: ParentComponent = (props) => {
  return <>{props.children}</>;
};

const DialogOverlay: Component<ComponentProps<"div">> = (props) => {
  const [local, others] = splitProps(props, ["class"]);
  return (
    <div
      class={cn(
        "fixed inset-0 z-50 bg-black/50 backdrop-blur-sm",
        local.class
      )}
      {...others}
    />
  );
};

const DialogContent: Component<ComponentProps<"div"> & DialogProps> = (props) => {
  const [local, others] = splitProps(props, ["class", "children", "open", "onOpenChange"]);
  return (
    <div
      class={cn(
        "fixed inset-x-4 top-1/2 z-50 w-auto -translate-y-1/2 rounded-xl border border-border bg-bg-secondary p-6 shadow-xl",
        local.class
      )}
      {...others}
    >
      {local.children}
      <button
        type="button"
        onClick={() => local.onOpenChange(false)}
        class="absolute right-3 top-3 rounded-md p-1 text-text-secondary hover:text-text-primary"
        aria-label="Close"
      >
        <span aria-hidden="true">x</span>
      </button>
    </div>
  );
};

const DialogHeader: ParentComponent<ComponentProps<"div">> = (props) => {
  const [local, others] = splitProps(props, ["class", "children"]);
  return (
    <div class={cn("flex flex-col space-y-1.5", local.class)} {...others}>
      {local.children}
    </div>
  );
};

const DialogTitle: Component<ComponentProps<"h2">> = (props) => {
  const [local, others] = splitProps(props, ["class"]);
  return (
    <h2 class={cn("text-lg font-semibold", local.class)} {...others} />
  );
};

const DialogDescription: Component<ComponentProps<"p">> = (props) => {
  const [local, others] = splitProps(props, ["class"]);
  return (
    <p class={cn("text-sm text-text-secondary", local.class)} {...others} />
  );
};

const DialogFooter: ParentComponent<ComponentProps<"div">> = (props) => {
  const [local, others] = splitProps(props, ["class", "children"]);
  return (
    <div class={cn("flex flex-col-reverse gap-2 sm:flex-row sm:justify-end", local.class)} {...others}>
      {local.children}
    </div>
  );
};

const useDialogState = (initial = false) => {
  const [open, setOpen] = createSignal(initial);
  return { open, setOpen };
};

interface ConfirmDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  onConfirm: () => void;
  isConfirmDisabled?: boolean;
}

const ConfirmDialog: Component<ConfirmDialogProps> = (props) => {
  const handleConfirm = () => {
    props.onConfirm();
  };

  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange}>
      <DialogPortal>
        <DialogOverlay onClick={() => props.onOpenChange(false)} />
        <DialogContent open={props.open} onOpenChange={props.onOpenChange}>
          <DialogHeader>
            <DialogTitle>{props.title}</DialogTitle>
            {props.description && (
              <DialogDescription>{props.description}</DialogDescription>
            )}
          </DialogHeader>
          <div class="mt-6">
            <DialogFooter>
              <Button
                variant="outline"
                onClick={() => props.onOpenChange(false)}
              >
                {props.cancelLabel ?? "Cancel"}
              </Button>
              <Button
                variant="destructive"
                onClick={handleConfirm}
                disabled={props.isConfirmDisabled}
              >
                {props.confirmLabel ?? "Confirm"}
              </Button>
            </DialogFooter>
          </div>
        </DialogContent>
      </DialogPortal>
    </Dialog>
  );
};

export {
  Dialog,
  DialogTrigger,
  DialogPortal,
  DialogOverlay,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
  ConfirmDialog,
  useDialogState,
};
