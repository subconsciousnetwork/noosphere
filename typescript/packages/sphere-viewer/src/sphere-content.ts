import { css, html, LitElement } from 'lit';
import { connect, watch } from 'lit-redux-watch';
import { customElement } from 'lit/decorators.js';
import { unsafeHTML } from 'lit/directives/unsafe-html.js';
import { sharedStyles } from './shared-styles.js';
import { store } from './state/store.js';

@customElement('sphere-content')
export class SphereFooter extends connect(store)(LitElement) {
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
    css`
      :host {
        display: block;
        min-height: 8em;
      }
    `,
  ];

  render() {
    if (!this.sphereId || !this.sphereVersion || !this.slug) {
      return html``;
    }

    let bodyContent;

    if (this.fileContents?.length) {
      bodyContent = unsafeHTML(this.fileContents);
    } else {
      bodyContent = html`<p class="empty">No body content found</p>`;
    }

    return html`
      <header class="card-header">
        <h1 class="label pad-b-sm">
          <a href="?id=${this.sphereId}&version=${this.sphereVersion}"
            >Sphere index</a
          >
        </h1>

        <!-- <a href="?id=${this.sphereId}&version=${this
          .sphereVersion}">🡸</a> -->
        <a
          href="?id=${this.sphereId}&version=${this.sphereVersion}&slug=${this
            .slug}"
          >/${this.slug}</a
        >
      </header>
      <div class="card-body">${bodyContent}</div>
    `;
  }
}
