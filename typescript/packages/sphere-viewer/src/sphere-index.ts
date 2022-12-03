import { css, html, LitElement } from 'lit';
import { connect, watch } from 'lit-redux-watch';
import { customElement } from 'lit/decorators.js';
import { sharedStyles } from './shared-styles.js';
import { store } from './state/store.js';

@customElement('sphere-index')
export class SphereIndex extends connect(store)(LitElement) {
  @watch('sphereViewer.sphereId')
  sphereId?: string;

  @watch('sphereViewer.sphereVersion')
  sphereVersion?: string;

  @watch('sphereViewer.sphereIndex')
  sphereIndex?: string[];

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
    if (!this.sphereId || !this.sphereVersion) {
      return html``;
    }

    let bodyContent;

    if (this.sphereIndex?.length) {
      let entries = this.sphereIndex.map(
        (entry) => html`
          <li class="row">
            <a
              class="row-button"
              href="?id=${this.sphereId}&version=${this
                .sphereVersion}&slug=${entry}"
              >/${entry}</a
            >
          </li>
        `
      );
      bodyContent = html`<ul class="group">
        ${entries}
      </ul>`;
    } else {
      bodyContent = html`<p class="empty">
        This sphere doesn't have any entries yet
      </p>`;
    }

    return html`
      <div class="card-body">
        <h1 class="label pad-b-sm">Sphere index</h1>
        ${bodyContent}
      </div>
    `;
  }
}
