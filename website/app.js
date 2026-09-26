import init, { generate_pdf } from "./pkg/record_sheet_wasm.js";

await init();

const form = document.getElementById("form");
const dateInput = document.getElementById("date");
const languageSelect = document.getElementById("language");
const titleInput = document.getElementById("title");
const logoInput = document.getElementById("logo");
const status = document.getElementById("status");

// Mirror the CLI default: empty date means "today", resolved in JS
// (the Rust side never calls now()).
const todayIso = () => new Date().toISOString().slice(0, 10);
dateInput.value = todayIso();

function setStatus(text, isError = false) {
  status.textContent = text;
  status.classList.toggle("error", isError);
}

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  const dateIso = dateInput.value || todayIso();
  const language = languageSelect.value;
  const title = titleInput.value.trim();
  const logoFile = logoInput.files[0];
  const logo = logoFile ? new Uint8Array(await logoFile.arrayBuffer()) : new Uint8Array();
  const filename = `record-sheet-${dateIso}.pdf`;

  try {
    const bytes = generate_pdf(dateIso, language, title === "" ? null : title, logo);
    const blob = new Blob([bytes], { type: "application/pdf" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = filename;
    link.click();
    URL.revokeObjectURL(url);
    setStatus(`Generated ${filename}`);
  } catch (error) {
    setStatus(error.message, true);
  }
});
