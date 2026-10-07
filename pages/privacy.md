<!-- Generated from privacy.html; edit the HTML source. -->

# Privacy Policy for Speaker Volume Bridge

**Effective date:**  October 4, 2026

Speaker Volume Bridge is an independent, community-developed desktop application published by Miguel.MS. This policy explains what information the application accesses and how that information is handled.

## Summary

Speaker Volume Bridge does not require an account, include advertising or analytics, or send telemetry to the publisher. It communicates directly with Sonos speakers on the user's local network and stores its settings and diagnostic logs locally on the user's computer.

## Information the application accesses

To provide volume synchronization, Speaker Volume Bridge accesses:

- information exposed by compatible Sonos devices on the local network, such as device identifiers, names, local network addresses, volume, and mute state;

- the selected Windows or macOS audio output identifier, volume, and mute state; and

- application preferences, including the selected devices, synchronization options, maximum volume, volume mapping, start-at-login choice, and diagnostic log level.

This information is used only to discover compatible devices, show their status, remember the user's choices, and synchronize volume and mute state.

## Local network communication

The application discovers and communicates with compatible Sonos devices over the user's local network using local discovery, control, and event-notification protocols. These communications are between the computer running the application and devices on that local network. The application does not send this information to the publisher or to an analytics or advertising service.

## Local storage and diagnostics

Application preferences are stored in a configuration file on the user's computer. Diagnostic log files are also stored locally and may contain technical error information and local device or network identifiers needed to diagnose a connection problem.

The application does not automatically upload configuration or log files. A user may choose to share diagnostic information when requesting support, for example by attaching it to a GitHub issue. Information shared in that way is handled by the service through which the user submits it and is subject to that service's privacy terms.

Users can reset application preferences from within the application. They can remove locally stored application data and logs by uninstalling the application and deleting any remaining application data, subject to the operating system's normal file-management behavior.

## Update checks

For recognized release installations, the application can request the public release information from GitHub at `api.github.com`. Previously published versions request the website feed at `svb.miguel.ms`. The request necessarily exposes ordinary web-hosting metadata such as the computer's public IP address, request time, API path, and network user agent to the hosting provider. It does not include speaker information, application settings, logs, a persistent installation identifier, analytics, or advertising data. Automatic checks can be disabled in Settings. The locally stored update record contains the preference and check, notification, and release-version state.

## Personal information and third parties

Speaker Volume Bridge does not ask for or intentionally collect names, email addresses, precise location, contacts, financial information, authentication credentials, audio content, or other personal content. It does not sell user information or disclose application data to advertisers or data brokers.

The application is not affiliated with, sponsored by, endorsed by, or supported by Sonos, Inc. Sonos devices and software are governed by Sonos's own terms and privacy practices.

## Security

The application limits device discovery and control to local-network addresses and validates received data. Users should install releases only from the Microsoft Store or the project's official GitHub repository and keep their operating system and Sonos devices updated.

## Children's privacy

The application is a general-purpose utility and is not directed to children. It does not knowingly collect personal information from children.

## Changes to this policy

This policy may be updated when the application's data practices change. The effective date above will be updated when a material revision is published.

## Contact and support

Questions about this policy or requests concerning application data can be submitted through the project's public issue tracker:

[Project issue tracker ↗](https://github.com/MiguelTVMS/speaker-volume-bridge/issues)

## Website privacy and tracking

This notice covers the website. The desktop application's local-only data practices described above are unchanged.

The site is hosted on GitHub Pages. GitHub may process visitor information, including IP addresses, to deliver and secure the site. See [GitHub’s privacy statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement) .

### Your consent choices

Optional website tracking is off by default. Google Tag Manager is loaded only after you enable analytics or advertising. You can accept all, reject optional tracking, or choose categories in Advanced. Reopen Privacy choices in the footer at any time to change or withdraw consent. Withdrawal reloads the page to stop previously loaded tags; it does not undo data already sent.

Analytics allows measurement of page visits and interactions. Advertising measurement allows advertising cookies and the sharing of advertising-related data with Google. Personalized advertising is separately optional and requires advertising measurement. Which measurements are collected depends on the tags configured in Google Tag Manager. Google may receive information such as your IP address, browser details, page URL, and interactions when permitted tags run. See [Google’s privacy policy](https://policies.google.com/privacy) .

### Storage and retention

Your choice is saved in this browser’s local storage under `svb-consent-v1` for up to 180 days, then requested again. This preference storage is used to remember your decision. If storage is unavailable, optional tags remain off on new visits. JavaScript-disabled visits do not load Google Tag Manager.

When you withdraw a category, the site attempts to remove accessible Google Analytics or advertising cookies for that category. It cannot delete third-party or HTTP-only cookies. Other service retention periods depend on the tags and Google service settings; the 180-day period applies only to your consent preference.

Download and support links lead to GitHub; the Microsoft Store link leads to Microsoft. Those services apply their own privacy terms. Questions about this website’s data practices can be raised through the [project issue tracker](https://github.com/MiguelTVMS/speaker-volume-bridge/issues) . Do not post private data in a public issue.

## Removing the former app

The app does not inspect running processes to look for the former Sonos Volume Bridge app. Before using the renamed app, quit and remove the former app and its startup entry manually to avoid competing speaker commands. See the [complete removal guide](https://svb.miguel.ms/guide/Removing-the-Old-App.html) .

[Speaker Volume Bridge](https://svb.miguel.ms/)

[Privacy policy](https://svb.miguel.ms/privacy.html)  [MIT license](https://svb.miguel.ms/license.txt)  [Miguel’s website ↗](https://miguel.ms)  [Source code ↗](https://github.com/MiguelTVMS/speaker-volume-bridge)

Independent software. Not affiliated with, sponsored by, endorsed by, or supported by Sonos. Sonos and related product names are trademarks of their respective owners and are used only to identify compatibility. This project contains no Sonos source code.

Made for the volume controls you already use.

{% if site.data.build %}

Version {{ site.data.build.version }} · {{ site.data.build.ref }} · {{ site.data.build.revision }}

{% endif %}
