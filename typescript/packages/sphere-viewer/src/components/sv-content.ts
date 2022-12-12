import { css, html, LitElement } from 'lit';
import { connect, watch } from 'lit-redux-watch';
import { customElement } from 'lit/decorators.js';
import { unsafeHTML } from 'lit/directives/unsafe-html.js';
import { sharedStyles } from '../styles/shared.js';
import { store } from '../state/store.js';
import { subtextStyles } from '../styles/subtext.js';
import { until } from 'lit/directives/until.js';
import { loadingIndicator } from '../styles/loading-indicator.js';

@customElement('sv-content')
export class SVContent extends connect(store)(LitElement) {
  @watch('sphereViewer.sphereId')
  sphereId?: string;

  @watch('sphereViewer.sphereVersion')
  sphereVersion?: string;

  @watch('sphereViewer.slug')
  slug?: string;

  @watch('sphereViewer.fileVersion')
  fileVersion?: string;

  @watch('sphereViewer.fileContents')
  fileContents?: Promise<string> | null;

  static styles = [
    sharedStyles,
    subtextStyles,
    loadingIndicator,
    css`
      .empty {
        display: flex;
        min-height: 8em;
        flex-direction: column;
        align-items: center;
        justify-content: center;
      }
      .slug {
        color: var(--color-text-secondary);
      }
    `,
  ];

  render() {
    const bodyContent = until(
      Promise.resolve(this.fileContents).then((contents) => {
        if (contents) {
          return html` ${unsafeHTML(contents)} `;
        } else {
          return html`<div class="empty">
            <p class="empty">No body content found</p>
          </div>`;
        }
      }),
      html`
        <div class="card-body center body-content message">
          <div class="loading-indicator"><span>Loading...</span></div>
        </div>
      `
    );

    return html` <div class="card-body">${bodyContent}</div> `;
  }
}
