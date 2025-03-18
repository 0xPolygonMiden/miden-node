// Populate the sidebar
//
// This is a script, and not included directly in the page, to control the total size of the book.
// The TOC contains an entry for each page, so if each page includes a copy of the TOC,
// the total size of the page becomes O(n**2).
class MDBookSidebarScrollbox extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        this.innerHTML = '<ol class="chapter"><li class="chapter-item expanded affix "><a href="index.html">Introduction</a></li><li class="chapter-item expanded affix "><li class="spacer"></li><li class="chapter-item expanded affix "><li class="part-title">Operator Guide</li><li class="chapter-item expanded "><a href="operator/index.html"><strong aria-hidden="true">1.</strong> Overview</a></li><li class="chapter-item expanded "><a href="operator/architecture.html"><strong aria-hidden="true">2.</strong> Architecture</a></li><li class="chapter-item expanded "><a href="operator/installation.html"><strong aria-hidden="true">3.</strong> Installation</a></li><li class="chapter-item expanded "><a href="operator/configuration.html"><strong aria-hidden="true">4.</strong> Configuraton and Usage</a></li><li class="chapter-item expanded "><a href="operator/monitoring.html"><strong aria-hidden="true">5.</strong> Monitoring</a></li><li class="chapter-item expanded "><a href="operator/versioning.html"><strong aria-hidden="true">6.</strong> Versioning</a></li><li class="chapter-item expanded affix "><li class="spacer"></li><li class="chapter-item expanded affix "><li class="part-title">Developer Guide</li><li class="chapter-item expanded "><a href="developer/index.html"><strong aria-hidden="true">7.</strong> Overview</a></li><li class="chapter-item expanded "><a href="developer/contributing.html"><strong aria-hidden="true">8.</strong> Contributing</a></li><li class="chapter-item expanded "><a href="developer/codebase.html"><strong aria-hidden="true">9.</strong> Navigating the codebase</a></li><li class="chapter-item expanded "><a href="developer/monitoring.html"><strong aria-hidden="true">10.</strong> Monitoring</a></li><li class="chapter-item expanded "><a href="developer/components.html"><strong aria-hidden="true">11.</strong> Components</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="developer/rpc.html"><strong aria-hidden="true">11.1.</strong> RPC</a></li><li class="chapter-item expanded "><a href="developer/store.html"><strong aria-hidden="true">11.2.</strong> Store</a></li><li class="chapter-item expanded "><a href="developer/block-producer.html"><strong aria-hidden="true">11.3.</strong> Block producer</a></li></ol></li><li class="chapter-item expanded "><a href="developer/oddities.html"><strong aria-hidden="true">12.</strong> Common issues other oddities</a></li><li class="chapter-item expanded affix "><li class="spacer"></li></ol>';
        // Set the current, active page, and reveal it if it's hidden
        let current_page = document.location.href.toString().split("#")[0];
        if (current_page.endsWith("/")) {
            current_page += "index.html";
        }
        var links = Array.prototype.slice.call(this.querySelectorAll("a"));
        var l = links.length;
        for (var i = 0; i < l; ++i) {
            var link = links[i];
            var href = link.getAttribute("href");
            if (href && !href.startsWith("#") && !/^(?:[a-z+]+:)?\/\//.test(href)) {
                link.href = path_to_root + href;
            }
            // The "index" page is supposed to alias the first chapter in the book.
            if (link.href === current_page || (i === 0 && path_to_root === "" && current_page.endsWith("/index.html"))) {
                link.classList.add("active");
                var parent = link.parentElement;
                if (parent && parent.classList.contains("chapter-item")) {
                    parent.classList.add("expanded");
                }
                while (parent) {
                    if (parent.tagName === "LI" && parent.previousElementSibling) {
                        if (parent.previousElementSibling.classList.contains("chapter-item")) {
                            parent.previousElementSibling.classList.add("expanded");
                        }
                    }
                    parent = parent.parentElement;
                }
            }
        }
        // Track and set sidebar scroll position
        this.addEventListener('click', function(e) {
            if (e.target.tagName === 'A') {
                sessionStorage.setItem('sidebar-scroll', this.scrollTop);
            }
        }, { passive: true });
        var sidebarScrollTop = sessionStorage.getItem('sidebar-scroll');
        sessionStorage.removeItem('sidebar-scroll');
        if (sidebarScrollTop) {
            // preserve sidebar scroll position when navigating via links within sidebar
            this.scrollTop = sidebarScrollTop;
        } else {
            // scroll sidebar to current active section when navigating via "next/previous chapter" buttons
            var activeSection = document.querySelector('#sidebar .active');
            if (activeSection) {
                activeSection.scrollIntoView({ block: 'center' });
            }
        }
        // Toggle buttons
        var sidebarAnchorToggles = document.querySelectorAll('#sidebar a.toggle');
        function toggleSection(ev) {
            ev.currentTarget.parentElement.classList.toggle('expanded');
        }
        Array.from(sidebarAnchorToggles).forEach(function (el) {
            el.addEventListener('click', toggleSection);
        });
    }
}
window.customElements.define("mdbook-sidebar-scrollbox", MDBookSidebarScrollbox);
