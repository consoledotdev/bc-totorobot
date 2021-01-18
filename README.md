# Console Basecamp Totorobot

A bot designed to run on Azure Functions and post things into the
[Console](https://console.dev) [Basecamp](https://basecamp.com) chat room. Runs
using a [Custom Request
Handler](https://docs.microsoft.com/en-us/azure/azure-functions/functions-custom-handlers)
which receives HTTP requests and responds like a web app.

## Methods
* `post_mailchimp_stats`: posts the latest stats from Mailchimp.

## Development

Uses [Rocket](https://rocket.rs/) so [requires Rust
nightly](https://rocket.rs/v0.4/guide/getting-started/). Set up a nightly
override on the directory once cloned:

```zsh
rustup override set nightly
```

Set environment variables (see below) with the relevant config then run:

```zsh
cargo run
```

to start the web server. Issue requests to the endpoints e.g.:

```zsh
curl -v http://localhost:3000/api/post_mailchimp_stats
```

### Unit tests

```zsh
cargo test
```

### Testing with Azure Functions

Requirements:

* [Azure Functions Core
  Tools](https://docs.microsoft.com/en-us/azure/azure-functions/functions-run-local#v2)

```zsh
# Create the local.settings.json file which will set the environment variables
# Only required on a fresh clone when the file doesn't exist
func azure functionapp fetch-app-settings bc-totorobot
func start
```

#### Trigger timer function
```zsh
curl -i -X POST -H "Content-Type:application/json" -d "{}" http://localhost:7071/admin/functions/post_mailchimp_stats
```

## Environment variables

* `TOTORO_MAILCHIMP_APIKEY`: Mailchimp API key.
* `TOTORO_MAILCHIMP_LIST_ID`: Mailchimp list ID string.
* `TOTORO_BASECAMP_BOTURL`: URL to post to in Basecamp.
* `TOTORO_PRODUCTION`: Set to any value when running in production.

## Deployment

[Azure
uses](https://docs.microsoft.com/en-us/azure/azure-functions/create-first-function-vs-code-other?tabs=rust%2Clinux#compile-the-custom-handler-for-azure)
the `x86_64-unknown-linux-musl` platform. Builds are done through [a dedicated
Docker container](https://github.com/clux/muslrust) that has various C
libraries built against musl.

Azure resources defined in [`main.bicep`](main.bicep).

### Manual

Requirements:

* Docker e.g. `sudo paman install docker`
* [Bicep](https://github.com/Azure/bicep/blob/main/docs/installing.md)
* [Azure CLI](https://docs.microsoft.com/en-us/cli/azure/install-azure-cli)
* [Azure Functions Core
  Tools](https://docs.microsoft.com/en-us/azure/azure-functions/functions-run-local#v2)

```zsh
docker pull clux/muslrust
docker run -v $PWD:/volume --rm -t clux/muslrust cargo build --release
mkdir bin # host.json configured to expect binary here
cp target/x86_64-unknown-linux-musl/release/totorobot bin/
bicep build ./main.bicep # generates main.json
az login
az deployment group create -f ./main.json -g bc-totorobot
func azure functionapp publish bc-totorobot
```

### Automatic

Uses [Azure
ARM](https://github.com/marketplace/actions/deploy-azure-resource-manager-arm-template)
and [Login](https://github.com/marketplace/actions/azure-login) GitHub actions
to deploy.

`AZURE_CREDENTIALS` created as per [the service principal
instructions](https://github.com/marketplace/actions/azure-login#configure-deployment-credentials).

```zsh
az ad sp create-for-rbac --name "bc-totorobot - GitHub" --sdk-auth --role contributor \
    --scopes /subscriptions/SUBSCRIPTIONID/resourceGroups/bc-totorobot
```
