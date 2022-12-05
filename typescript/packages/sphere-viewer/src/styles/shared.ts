import { css } from 'lit';

export const sharedStyles = css`
  * {
    border: 0;
    margin: 0;
    padding: 0;
    box-sizing: border-box;
    list-style: none;
  }

  a {
    color: var(--color-purple);
    text-decoration: none;
  }

  .mono {
    font-family: var(--font-mono);
  }

  .container {
    margin: 0 auto;
    max-width: 800px;
  }

  .pad-container {
    padding: 64px 16px;
  }

  .border {
    border-bottom: 1px solid var(--color-border);
  }

  .pad-box {
    padding: var(--pad);
  }

  .pad-b {
    padding-bottom: var(--pad);
  }

  .pad-b-sm {
    padding-bottom: var(--pad-sm);
  }

  .card {
    background: #fff;
    border-radius: var(--radius-lg);
    overflow: hidden;
    box-shadow: 4px 0 24px rgba(0, 0, 0, 0.07);
  }

  .card-header {
    padding: 24px;
    border-bottom: 1px solid var(--color-border);
  }

  .card-body {
    padding: 24px;
  }

  .card-footer {
    background: var(--color-background-tertiary);
    padding: 24px;
  }

  .content {
    max-width: 36em;
  }

  .capsule {
    background: var(--color-background-tertiary);
    display: inline-block;
    border-radius: 32px;
    line-height: 32px;
    padding: 4px 16px;
    display: inline-block;
    text-decoration: none;
  }

  .blocks {
    list-style: none;
    margin: 0;
  }

  .blocks > li {
    padding-bottom: var(--pad-block);
  }

  .blocks > li:last-child {
    padding-bottom: 0;
  }

  .h1 {
    font-size: var(--text-h1-size);
    line-height: var(--text-h1-line);
    max-width: 20em;
  }

  .color-text {
    color: var(--color-text);
  }

  .color-secondary {
    color: var(--color-text-secondary);
  }

  .small {
    font-size: var(--text-caption-size);
    line-height: var(--text-caption-line);
  }

  .caption {
    font-size: var(--text-caption-size);
    line-height: var(--text-caption-line);
  }

  .label {
    color: var(--color-text-secondary);
    font-weight: bold;
    font-size: var(--text-label-size);
    line-height: var(--text-label-line);
    text-transform: uppercase;
  }

  .trunc {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .group {
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    overflow: hidden;
  }

  .row {
    border-top: 1px solid var(--color-border);
  }

  .row:first-child {
    border-top: 0;
  }

  .pad-row {
    padding: 16px;
  }

  .row-button {
    background-color: transparent;
    transition: background-color 400ms ease-out;
    display: block;
    padding: 16px;
    text-decoration: none;
  }

  .row-button:hover {
    background-color: var(--color-selected);
  }

  .button {
    background: var(--color-blush);
    border-radius: var(--radius-sm);
    color: var(--color-purple);
    font-weight: bold;
    font-size: var(--text-body-size);
    line-height: var(--text-body-line);
    display: inline-block;
    white-space: nowrap;
    text-decoration: none;
    padding: 12px 20px;
  }

  .button-full {
    display: block;
    text-align: center;
  }

  .block {
    display: block;
  }

  .nav {
    display: flex;
    gap: 20px;
    align-items: center;
    padding-bottom: 24px;
  }

  .nav-end {
    margin-left: auto;
  }

  .center {
    text-align: center;
  }

  .empty {
    text-align: center;
    font-style: italic;
  }
`;
