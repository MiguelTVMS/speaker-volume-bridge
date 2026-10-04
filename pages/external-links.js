(() => {
  const currentOrigin = window.location.origin;

  for (const link of document.querySelectorAll("a[href]")) {
    let destination;
    try {
      destination = new URL(link.href, window.location.href);
    } catch {
      continue;
    }

    if (!/^https?:$/.test(destination.protocol) || destination.origin === currentOrigin) {
      continue;
    }

    link.target = "_blank";
    const relationships = new Set((link.rel || "").split(/\s+/).filter(Boolean));
    relationships.add("noopener");
    relationships.add("noreferrer");
    link.rel = [...relationships].join(" ");
  }
})();
