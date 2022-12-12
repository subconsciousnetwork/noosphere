import { css } from 'lit';

export const subtextStyles = css`
  .subtext {
    display: flex;
    flex-direction: column;
    gap: var(--pad-block);
  }

  .subtext blockquote {
    font-style: italic;
  }

  .subtext .block-list {
    padding-left: 1em;
  }

  .subtext .block-list:before {
    color: var(--color-text-secondary);
    content: '-';
    position: absolute;
    margin-left: -1em;
  }

  .subtext .block-transcludes:not(:first-child) {
    margin-top: 1em;
  }

  .subtext .transclude {
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    display: block;
    padding: 1em;
  }

  .subtext .block:last-child .block-transcludes {
    margin-bottom: 0;
  }

  .subtext .block-transcludes > .transclude-item {
    padding: 16px;
    border-radius: 16px;
    background-color: var(--color-background-tertiary);
  }

  .subtext .block-transcludes {
    display: flex;
    flex-direction: column;
    gap: var(--pad-block);
  }

  .subtext .transclude-format-text {
    display: flex;
    flex-direction: column;
    gap: var(--pad-xs);
  }

  .subtext .transclude-format-text > .excerpt {
    color: var(--color-text);
  }

  .subtext .transclude-format-text > .link-text {
    color: var(--color-text-secondary);
  }

  .subtext .transclude-format-text > .title {
    font-weight: bold;
  }

  .subtext .block-blank {
    display: block;
    position: relative;
    margin-bottom: 1em;
  }

  .subtext .block-header {
    font-size: 1em;
    font-weight: bold;
    margin: 0;
  }
`;
