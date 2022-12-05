import { css } from 'lit';

export const subtextStyles = css`
  .subtext > .block:not(:last-child) {
    margin-bottom: 1em;
  }

  .subtext blockquote {
    font-style: italic;
    padding-left: 1em;
  }

  .subtext .block-transcludes {
    margin-top: 1em;
  }

  .subtext .block-transcludes > .transclude-item {
    padding: 16px;
    border-radius: 16px;
    background-color: var(--color-background-tertiary);
  }

  .subtext .block-transcludes > .transclude-item:not(:last-child) {
    margin-bottom: 1em;
  }

  .subtext .transclude-format-text {
    font-size: 0.85em;
  }

  .subtext .transclude-format-text > * {
    display: block;
  }

  .subtext .transclude-format-text > .excerpt {
    font-style: italic;
  }

  .subtext .transclude-format-text > .link-text {
    font-weight: bold;
  }
`;
