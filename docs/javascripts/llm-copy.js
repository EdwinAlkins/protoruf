// Adds a "Copy this prompt" button to LLM-prompt pages.
//
// A page opts in by including `<div class="llm-copy-mount"></div>` near its top.
// The button copies the page's *raw Markdown* (best for pasting into an LLM). It
// derives the raw source URL from Material's "edit" link, and falls back to the
// rendered text when that source can't be fetched (e.g. local `mkdocs serve`).

function initLlmCopy() {
  document.querySelectorAll(".llm-copy-mount").forEach((mount) => {
    if (mount.dataset.ready) return; // guard against double-init on instant nav
    mount.dataset.ready = "1";

    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "md-button md-button--primary llm-copy-btn";
    btn.textContent = "📋 Copy this prompt";
    mount.appendChild(btn);

    btn.addEventListener("click", async () => {
      const original = btn.textContent;
      try {
        const text = await getPageMarkdown();
        await navigator.clipboard.writeText(text);
        btn.textContent = "✅ Copied!";
      } catch (e) {
        btn.textContent = "⚠️ Copy failed";
        console.error("llm-copy:", e);
      }
      setTimeout(() => (btn.textContent = original), 2000);
    });
  });
}

async function getPageMarkdown() {
  // Prefer the raw Markdown source, derived from the "edit this page" link.
  const edit = document.querySelector('a[href*="/edit/"]');
  if (edit) {
    const raw = edit.href
      .replace("github.com", "raw.githubusercontent.com")
      .replace("/edit/", "/");
    try {
      const res = await fetch(raw);
      if (res.ok) {
        const md = await res.text();
        // Drop the opt-in mount marker so it doesn't leak into the prompt.
        return md.replace(/^.*llm-copy-mount.*\r?\n?/gm, "").trim();
      }
    } catch (_) {
      /* fall through to rendered-text fallback */
    }
  }
  // Fallback: copy the rendered article text (works offline / without a source).
  const article = document.querySelector(".md-content__inner .md-typeset");
  return article ? article.innerText.trim() : document.body.innerText.trim();
}

// Support Material's instant navigation (re-runs on each page swap) and plain loads.
if (typeof window.document$ !== "undefined") {
  window.document$.subscribe(initLlmCopy);
} else {
  document.addEventListener("DOMContentLoaded", initLlmCopy);
}
