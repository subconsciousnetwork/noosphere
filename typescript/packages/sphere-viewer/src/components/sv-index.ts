import { css, html, LitElement } from 'lit';
import { connect, watch } from 'lit-redux-watch';
import { customElement } from 'lit/decorators.js';
import { sharedStyles } from '../styles/shared.js';
import { store } from '../state/store.js';
import { loadingIndicator } from '../styles/loading-indicator.js';
import { until } from 'lit/directives/until.js';

@customElement('sv-index')
export class SVIndex extends connect(store)(LitElement) {
  @watch('sphereViewer.sphereId')
  sphereId?: string;

  @watch('sphereViewer.sphereVersion')
  sphereVersion?: string;

  @watch('sphereViewer.sphereIndex')
  sphereIndex?: Promise<string[]>;

  static styles = [sharedStyles, loadingIndicator];

  render() {
    const bodyContent = until(
      this.sphereIndex?.then((index) => {
        if (index) {
          let entries = index.map(
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
          return html` <h1 class="label pad-b-sm">Sphere index</h1>
            <ul class="group">
              ${entries}
            </ul>`;
        } else {
          return html`<p class="empty">
            This sphere doesn't have any entries yet
          </p>`;
        }
      }),
      html` <div class="loading-indicator"><span>Loading...</span></div> `
    );

    return html` <div class="card-body">${bodyContent}</div> `;
  }
}
