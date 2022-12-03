import './components/sv-content.js';
import './components/sv-footer.js';
import './components/sv-index.js';
import './components/sv-header.js';

import { customElement } from 'lit/decorators.js';
import { LitElement, html, css } from 'lit';
import { connect, watch } from 'lit-redux-watch';
import { store } from './state/store.js';

import { sharedStyles } from './styles/shared.js';
import { until } from 'lit/directives/until.js';
import { loadingIndicator } from './styles/loading-indicator.js';

@customElement('sphere-viewer')
export class SphereViewer extends connect(store)(LitElement) {
  @watch('sphereViewer.sphereId')
  sphereId?: string;

  @watch('sphereViewer.sphereVersion')
  sphereVersion?: string;

  @watch('sphereViewer.slug')
  slug?: string;

  @watch('sphereViewer.fileContents')
  fileContents?: Promise<string>;

  @watch('sphereViewer.loading')
  loading?: Promise<void>;

  static styles = [
    sharedStyles,
    loadingIndicator,
    css`
      .body-content {
        display: block;
        min-height: 8em;
      }
      .body-content.message {
        display: flex;
        flex-direction: column;
        justify-content: center;
        align-items: center;
      }
    `,
  ];

  render() {
    let bodyContent = until(
      this.loading?.then(() => {
        console.log(this.sphereId, this.sphereVersion);
        if (this.sphereId && this.sphereVersion) {
          if (this.slug) {
            return html`<sv-content></sv-content>
              <sv-footer></sv-footer> `;
          } else {
            return html`<sv-index></sv-index>
              <sv-footer></sv-footer> `;
          }
        } else {
          console.log('WAT');
          return html`<div class="card-body center body-content message">
            <p>No sphere address information specified</p>
          </div>`;
        }
      }),
      html`<div class="card-body center body-content message">
        <div class="loading-indicator"><span>Loading...</span></div>
      </div>`
    );

    return html`
      <div class="container pad-container">
        <article class="card">
          <sv-header></sv-header>
          ${bodyContent}
        </article>
      </div>
    `;
  }
}
