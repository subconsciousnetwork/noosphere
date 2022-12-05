import { css, html, LitElement } from 'lit';
import { connect, watch } from 'lit-redux-watch';
import { customElement } from 'lit/decorators.js';
import { unsafeHTML } from 'lit/directives/unsafe-html.js';
import { sharedStyles } from '../styles/shared.js';
import { store } from '../state/store.js';
import { subtextStyles } from '../styles/subtext.js';

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
  fileContents?: string;

  static styles = [
    sharedStyles,
    subtextStyles,
    css`
      :host {
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
    if (!this.sphereId || !this.sphereVersion || !this.slug) {
      return html``;
    }

    let bodyContent;

    if (this.fileContents?.length) {
      bodyContent = html` ${unsafeHTML(this.fileContents)} `;
    } else {
      bodyContent = html`<p class="empty">No body content found</p>`;
    }

    return html` <div class="card-body">${bodyContent}</div> `;
  }
}
