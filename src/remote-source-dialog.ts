// Wires up the "+ 远程仓库" native <dialog> form: owns its own DOM elements
// and submit handling, and hands the newly created Source back to the
// caller once import succeeds.
import { addRemoteSource, type Source } from "./api";

export function initRemoteSourceDialog(onImported: (source: Source) => Promise<void>): void {
  const openButton = document.querySelector<HTMLButtonElement>("#add-remote-button")!;
  const dialog = document.querySelector<HTMLDialogElement>("#remote-source-dialog")!;
  const form = document.querySelector<HTMLFormElement>("#remote-source-form")!;
  const cancelButton = document.querySelector<HTMLButtonElement>("#remote-dialog-cancel")!;
  const submitButton = document.querySelector<HTMLButtonElement>("#remote-dialog-submit")!;
  const errorEl = document.querySelector<HTMLElement>("#remote-dialog-error")!;
  const ownerInput = document.querySelector<HTMLInputElement>("#remote-owner")!;
  const repoInput = document.querySelector<HTMLInputElement>("#remote-repo")!;
  const branchInput = document.querySelector<HTMLInputElement>("#remote-branch")!;
  const pathInput = document.querySelector<HTMLInputElement>("#remote-path")!;
  const nameInput = document.querySelector<HTMLInputElement>("#remote-name")!;
  const translateToInput = document.querySelector<HTMLInputElement>("#remote-translate-to")!;

  openButton.addEventListener("click", () => {
    errorEl.hidden = true;
    dialog.showModal();
  });
  cancelButton.addEventListener("click", () => dialog.close());
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    void submit();
  });

  async function submit(): Promise<void> {
    errorEl.hidden = true;
    submitButton.disabled = true;
    const originalLabel = submitButton.textContent;
    submitButton.textContent = "导入中…";
    try {
      const source = await addRemoteSource({
        owner: ownerInput.value.trim(),
        repo: repoInput.value.trim(),
        branch: branchInput.value.trim() || "main",
        path: pathInput.value.trim(),
        name: nameInput.value.trim() || undefined,
        translateTo: translateToInput.value.trim() || undefined,
      });
      form.reset();
      branchInput.value = "main";
      dialog.close();
      await onImported(source);
    } catch (err) {
      errorEl.textContent = String(err);
      errorEl.hidden = false;
    } finally {
      submitButton.disabled = false;
      submitButton.textContent = originalLabel;
    }
  }
}
