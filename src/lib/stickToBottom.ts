/** Scrollable output panels: follow new content at the bottom until the user scrolls up. */
export function stickToBottom(node: HTMLElement, options?: { threshold?: number }) {
  let stuck = true;
  const threshold = options?.threshold ?? 40;

  function isAtBottom(): boolean {
    return node.scrollHeight - node.scrollTop - node.clientHeight <= threshold;
  }

  function scrollToBottom() {
    node.scrollTop = node.scrollHeight;
  }

  function onScroll() {
    stuck = isAtBottom();
  }

  function maybeScroll() {
    if (stuck) {
      requestAnimationFrame(scrollToBottom);
    }
  }

  const mutationObserver = new MutationObserver(maybeScroll);
  mutationObserver.observe(node, { childList: true, subtree: true, characterData: true });

  const resizeObserver = new ResizeObserver(maybeScroll);
  resizeObserver.observe(node);

  node.addEventListener('scroll', onScroll, { passive: true });
  maybeScroll();

  return {
    destroy() {
      mutationObserver.disconnect();
      resizeObserver.disconnect();
      node.removeEventListener('scroll', onScroll);
    },
  };
}
