const markdownIt = require("markdown-it");
const markdownItAnchor = require("markdown-it-anchor");
const slugify = require("slugify");

// A plug-in for markdown-it that adds a `target="_blank"` attribute to any
// anchors whose `href` points to an external origin
function newTabExternalLinks(md, options) {
  let {
    skipOrigins,
    localOrigin
  } = options;

  if (!Array.isArray(skipOrigins)) {
    skipOrigins = [];
  }

  md.core.ruler.push('external_anchors', (state) => {
    for (const token of state.tokens) {
      if (token.type == "inline") {
        for (const inlineToken of token.children) {
          if (inlineToken.type == "link_open") {
            const attrs = inlineToken.attrs.slice();

            for (const [attribute, value] of attrs) {
              if (attribute === "href") {
                let hrefUrl;
                try {
                  hrefUrl = new URL(value);
                } catch(e) {
                  break;
                }
                let transformToTargetBlank = true;
                if (hrefUrl.origin === localOrigin.origin) {
                  transformToTargetBlank = false;
                } else {
                  for (const skipOriginUrl of skipOrigins) {
                    if (hrefUrl.origin === skipOriginUrl.origin) {
                      transformToTargetBlank = false;
                      break;
                    }
                  }
                }
                if (transformToTargetBlank) {
                  inlineToken.attrs.push(["target", "_blank"]);
                }
                break;
              }
            }
          }
        }
      }
    }
  });
};

// Configure hash anchors that are automatically added to headings
const markdownItAnchorOptions = {
  level: [1, 2, 3],
  slugify: (str) =>
    slugify(str, {
      lower: true,
      strict: true,
      remove: /["]/g,
    }),
  tabIndex: false,
  permalink: markdownItAnchor.permalink.linkInsideHeader({
    symbol: `
      <span aria-label="Jump to heading">#</span>
    `,
    placement: 'after'
  })
};

let markdownLibrary = markdownIt({
  html: true,
}).use(markdownItAnchor, markdownItAnchorOptions).use(newTabExternalLinks, {
  localOrigin: new URL("http://localhost:8928"),
  skipOrigins: []
});


module.exports = function (eleventyConfig) {
  eleventyConfig.setLibrary("md", markdownLibrary);
  eleventyConfig.addPassthroughCopy('_static/styles');
  eleventyConfig.addPassthroughCopy('_static/images'); 

  eleventyConfig.setServerOptions({
    liveReload: true,
    domDiff: true,
    port: 8928,
    watch: [],
    showAllHosts: false,
    https: {},
    encoding: "utf-8",
    showVersion: false,
  });
}