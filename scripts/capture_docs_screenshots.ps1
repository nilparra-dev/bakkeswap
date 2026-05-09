param(
  [int]$Port = 9224,
  [string]$OutputDir = $(Join-Path $PSScriptRoot "..\docs\screenshots")
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$script:MessageId = 0

function ConvertTo-JsonLiteral {
  param([Parameter(Mandatory = $true)] [object]$Value)

  return ($Value | ConvertTo-Json -Compress)
}

function Get-DebuggerTarget {
  param([Parameter(Mandatory = $true)] [int]$DebuggerPort)

  $deadline = [DateTime]::UtcNow.AddSeconds(20)

  while ([DateTime]::UtcNow -lt $deadline) {
    try {
      $targets = @(Invoke-RestMethod -Uri "http://127.0.0.1:$DebuggerPort/json/list")
      $target = $targets | Where-Object { $_.type -eq "page" -and $_.webSocketDebuggerUrl } | Select-Object -First 1
      if ($target) {
        return $target.webSocketDebuggerUrl
      }
    } catch {
    }
  }

  throw "No page target was exposed on port $DebuggerPort. Start the Tauri app with WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=$DebuggerPort first."
}

function Connect-Debugger {
  param([Parameter(Mandatory = $true)] [string]$WebSocketUrl)

  $client = [System.Net.WebSockets.ClientWebSocket]::new()
  [void]$client.ConnectAsync([Uri]$WebSocketUrl, [Threading.CancellationToken]::None).GetAwaiter().GetResult()
  return $client
}

function Read-DebuggerMessage {
  param([Parameter(Mandatory = $true)] [System.Net.WebSockets.ClientWebSocket]$Client)

  $buffer = New-Object byte[] 65536
  $builder = New-Object System.Text.StringBuilder

  do {
    $segment = New-Object "System.ArraySegment[byte]" -ArgumentList (, $buffer)
    $result = $Client.ReceiveAsync($segment, [Threading.CancellationToken]::None).GetAwaiter().GetResult()

    if ($result.MessageType -eq [System.Net.WebSockets.WebSocketMessageType]::Close) {
      throw "The WebView2 debugger connection closed before the capture completed."
    }

    [void]$builder.Append([Text.Encoding]::UTF8.GetString($buffer, 0, $result.Count))
  } while (-not $result.EndOfMessage)

  return $builder.ToString()
}

function Invoke-Debugger {
  param(
    [Parameter(Mandatory = $true)] [System.Net.WebSockets.ClientWebSocket]$Client,
    [Parameter(Mandatory = $true)] [string]$Method,
    [hashtable]$Params = @{}
  )

  $script:MessageId += 1
  $messageId = $script:MessageId
  $payload = @{ id = $messageId; method = $Method; params = $Params } | ConvertTo-Json -Depth 20 -Compress
  $bytes = [Text.Encoding]::UTF8.GetBytes($payload)
  $segment = New-Object "System.ArraySegment[byte]" -ArgumentList (, $bytes)
  [void]$Client.SendAsync($segment, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, [Threading.CancellationToken]::None).GetAwaiter().GetResult()

  while ($true) {
    $raw = Read-DebuggerMessage -Client $Client

    try {
      $response = $raw | ConvertFrom-Json -ErrorAction Stop
    } catch {
      continue
    }

    $idProperty = $response.PSObject.Properties["id"]
    if ($idProperty -and [int]$idProperty.Value -eq $messageId) {
      $errorProperty = $response.PSObject.Properties["error"]
      if ($errorProperty -and $errorProperty.Value) {
        throw "Debugger call failed for ${Method}: $($errorProperty.Value.message)"
      }

      $resultProperty = $response.PSObject.Properties["result"]
      if ($resultProperty) {
        return $resultProperty.Value
      }

      return $null
    }
  }
}

function Invoke-JsValue {
  param(
    [Parameter(Mandatory = $true)] [System.Net.WebSockets.ClientWebSocket]$Client,
    [Parameter(Mandatory = $true)] [string]$Expression,
    [switch]$AwaitPromise
  )

  $result = Invoke-Debugger -Client $Client -Method "Runtime.evaluate" -Params @{
      expression    = $Expression
      awaitPromise  = [bool]$AwaitPromise
      returnByValue = $true
    }

  $exceptionProperty = $result.PSObject.Properties["exceptionDetails"]
  if ($exceptionProperty -and $exceptionProperty.Value) {
    throw "JavaScript evaluation failed: $($exceptionProperty.Value.text)"
  }

  return $result.result.value
}

function Wait-ForCondition {
  param(
    [Parameter(Mandatory = $true)] [System.Net.WebSockets.ClientWebSocket]$Client,
    [Parameter(Mandatory = $true)] [string]$Condition,
    [Parameter(Mandatory = $true)] [string]$Description,
    [int]$TimeoutMs = 15000
  )

  $scriptTemplate = @'
(async () => {
  const deadline = performance.now() + __TIMEOUT__;
  while (performance.now() < deadline) {
    try {
      if (__CONDITION__) {
        return true;
      }
    } catch (error) {
    }
    await new Promise((resolve) => requestAnimationFrame(() => resolve()));
  }
  return false;
})()
'@

  $scriptText = $scriptTemplate.Replace("__TIMEOUT__", [string]$TimeoutMs).Replace("__CONDITION__", $Condition)
  $ready = Invoke-JsValue -Client $Client -Expression $scriptText -AwaitPromise

  if (-not $ready) {
    throw "Timed out waiting for $Description."
  }
}

function Invoke-JsAction {
  param(
    [Parameter(Mandatory = $true)] [System.Net.WebSockets.ClientWebSocket]$Client,
    [Parameter(Mandatory = $true)] [string]$Script,
    [Parameter(Mandatory = $true)] [string]$Description
  )

  $success = Invoke-JsValue -Client $Client -Expression $Script
  if (-not $success) {
    throw "Unable to complete action: $Description."
  }
}

function Open-Page {
  param(
    [Parameter(Mandatory = $true)] [System.Net.WebSockets.ClientWebSocket]$Client,
    [Parameter(Mandatory = $true)] [string]$Label
  )

  $labelJson = ConvertTo-JsonLiteral -Value $Label
  $scriptText = @'
(() => {
  const wanted = __LABEL__.toLowerCase();
  const button = Array.from(document.querySelectorAll('.nav button')).find((entry) => entry.innerText.toLowerCase().includes(wanted));
  if (!button) {
    return false;
  }
  button.click();
  return true;
})()
'@

  $scriptText = $scriptText.Replace("__LABEL__", $labelJson)
  Invoke-JsAction -Client $Client -Script $scriptText -Description "open page $Label"

  $condition = "document.querySelector('.topbar h2')?.textContent?.toLowerCase().includes($labelJson.toLowerCase())"
  Wait-ForCondition -Client $Client -Condition $condition -Description "page heading $Label"
}

function Set-FieldValue {
  param(
    [Parameter(Mandatory = $true)] [System.Net.WebSockets.ClientWebSocket]$Client,
    [Parameter(Mandatory = $true)] [string]$Label,
    [Parameter(Mandatory = $true)] [string]$Value
  )

  $labelJson = ConvertTo-JsonLiteral -Value $Label
  $valueJson = ConvertTo-JsonLiteral -Value $Value
  $scriptText = @'
(() => {
  const wanted = __LABEL__.toLowerCase();
  const field = Array.from(document.querySelectorAll('label.field')).find((entry) => {
    const caption = entry.querySelector('span');
    return caption && caption.textContent.toLowerCase().includes(wanted);
  });
  if (!field) {
    return false;
  }
  const input = field.querySelector('input');
  if (!input) {
    return false;
  }
  input.focus();
  input.value = __VALUE__;
  input.dispatchEvent(new Event('input', { bubbles: true }));
  input.dispatchEvent(new Event('change', { bubbles: true }));
  return true;
})()
'@

  $scriptText = $scriptText.Replace("__LABEL__", $labelJson).Replace("__VALUE__", $valueJson)
  Invoke-JsAction -Client $Client -Script $scriptText -Description "set field $Label"
}

function Click-Button {
  param(
    [Parameter(Mandatory = $true)] [System.Net.WebSockets.ClientWebSocket]$Client,
    [Parameter(Mandatory = $true)] [string]$Label
  )

  $labelJson = ConvertTo-JsonLiteral -Value $Label
  $scriptText = @'
(() => {
  const wanted = __LABEL__.toLowerCase();
  const buttons = Array.from(document.querySelectorAll('button')).filter((entry) => !entry.disabled);
  const button = buttons.find((entry) => entry.innerText.trim().toLowerCase() === wanted)
    || buttons.find((entry) => entry.innerText.trim().toLowerCase().includes(wanted));
  if (!button) {
    return false;
  }
  button.click();
  return true;
})()
'@

  $scriptText = $scriptText.Replace("__LABEL__", $labelJson)
  Invoke-JsAction -Client $Client -Script $scriptText -Description "click button $Label"
}

function Wait-ForSearchResult {
  param(
    [Parameter(Mandatory = $true)] [System.Net.WebSockets.ClientWebSocket]$Client,
    [Parameter(Mandatory = $true)] [string]$Side,
    [Parameter(Mandatory = $true)] [string]$ProductMarker
  )

  $sideJson = ConvertTo-JsonLiteral -Value $Side
  $productJson = ConvertTo-JsonLiteral -Value $ProductMarker
  $conditionTemplate = @'
(() => {
  const wanted = __SIDE__.toLowerCase();
  const panel = Array.from(document.querySelectorAll('.search-panel')).find((entry) => entry.innerText.toLowerCase().includes(wanted));
  if (!panel) {
    return false;
  }
  return Array.from(panel.querySelectorAll('.result-list button')).some((entry) => entry.innerText.includes(__PRODUCT__));
})()
'@

  $condition = $conditionTemplate.Replace("__SIDE__", $sideJson).Replace("__PRODUCT__", $productJson)
  Wait-ForCondition -Client $Client -Condition $condition -Description "$Side search result $ProductMarker"
}

function Click-SearchResult {
  param(
    [Parameter(Mandatory = $true)] [System.Net.WebSockets.ClientWebSocket]$Client,
    [Parameter(Mandatory = $true)] [string]$Side,
    [Parameter(Mandatory = $true)] [string]$ProductMarker
  )

  $sideJson = ConvertTo-JsonLiteral -Value $Side
  $productJson = ConvertTo-JsonLiteral -Value $ProductMarker
  $scriptText = @'
(() => {
  const wanted = __SIDE__.toLowerCase();
  const panel = Array.from(document.querySelectorAll('.search-panel')).find((entry) => entry.innerText.toLowerCase().includes(wanted));
  if (!panel) {
    return false;
  }
  const button = Array.from(panel.querySelectorAll('.result-list button')).find((entry) => entry.innerText.includes(__PRODUCT__));
  if (!button) {
    return false;
  }
  button.click();
  return true;
})()
'@

  $scriptText = $scriptText.Replace("__SIDE__", $sideJson).Replace("__PRODUCT__", $productJson)
  Invoke-JsAction -Client $Client -Script $scriptText -Description "select $Side result $ProductMarker"
}

function Click-FirstRowAction {
  param([Parameter(Mandatory = $true)] [System.Net.WebSockets.ClientWebSocket]$Client)

  $scriptText = @'
(() => {
  const button = document.querySelector('.swap-table .row-action:not([disabled])');
  if (!button) {
    return false;
  }
  button.click();
  return true;
})()
'@

  return [bool](Invoke-JsValue -Client $Client -Expression $scriptText)
}

function Scroll-ToTop {
  param([Parameter(Mandatory = $true)] [System.Net.WebSockets.ClientWebSocket]$Client)

  [void](Invoke-JsValue -Client $Client -Expression "(() => { window.scrollTo(0, 0); return true; })()")
}

function Scroll-ToSelector {
  param(
    [Parameter(Mandatory = $true)] [System.Net.WebSockets.ClientWebSocket]$Client,
    [Parameter(Mandatory = $true)] [string]$Selector
  )

  $selectorJson = ConvertTo-JsonLiteral -Value $Selector
  $scriptText = @'
(() => {
  const node = document.querySelector(__SELECTOR__);
  if (!node) {
    return false;
  }
  node.scrollIntoView({ block: 'start', inline: 'nearest' });
  return true;
})()
'@

  $scriptText = $scriptText.Replace("__SELECTOR__", $selectorJson)
  Invoke-JsAction -Client $Client -Script $scriptText -Description "scroll to $Selector"
}

function Save-Screenshot {
  param(
    [Parameter(Mandatory = $true)] [System.Net.WebSockets.ClientWebSocket]$Client,
    [Parameter(Mandatory = $true)] [string]$FileName
  )

  $result = Invoke-Debugger -Client $Client -Method "Page.captureScreenshot" -Params @{
      format                = "png"
      captureBeyondViewport = $false
      fromSurface           = $true
    }

  $targetPath = Join-Path $OutputDir $FileName
  [IO.File]::WriteAllBytes($targetPath, [Convert]::FromBase64String($result.data))
  Write-Host "Captured $targetPath"
}

function Prepare-QuickSwapState {
  param([Parameter(Mandatory = $true)] [System.Net.WebSockets.ClientWebSocket]$Client)

  Open-Page -Client $Client -Label "Quick Swap"
  Set-FieldValue -Client $Client -Label "Search TARGET" -Value "1001"
  Wait-ForSearchResult -Client $Client -Side "TARGET" -ProductMarker "#1001"
  Click-SearchResult -Client $Client -Side "TARGET" -ProductMarker "#1001"

  Set-FieldValue -Client $Client -Label "Search SOURCE" -Value "1002"
  Wait-ForSearchResult -Client $Client -Side "SOURCE" -ProductMarker "#1002"
  Click-SearchResult -Client $Client -Side "SOURCE" -ProductMarker "#1002"

  Click-Button -Client $Client -Label "Create plan"
  Wait-ForCondition -Client $Client -Condition "document.body.innerText.includes('source_decal_on_target_decal')" -Description "plan profile name"

  Click-Button -Client $Client -Label "Build"
  Wait-ForCondition -Client $Client -Condition "Array.from(document.querySelectorAll('button')).some((entry) => entry.innerText.trim().toLowerCase().includes('preview install') && !entry.disabled)" -Description "enabled preview action"

  Click-Button -Client $Client -Label "Preview install"
  Wait-ForCondition -Client $Client -Condition "document.body.innerText.includes('INSTALL source_decal_on_target_decal') || document.body.innerText.toLowerCase().includes('preview ready')" -Description "install preview state"
}

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

$debuggerUrl = Get-DebuggerTarget -DebuggerPort $Port
$client = Connect-Debugger -WebSocketUrl $debuggerUrl

try {
  [void](Invoke-Debugger -Client $client -Method "Page.enable")
  [void](Invoke-Debugger -Client $client -Method "Runtime.enable")
  [void](Invoke-Debugger -Client $client -Method "Page.bringToFront")
  [void](Invoke-Debugger -Client $client -Method "Emulation.setDeviceMetricsOverride" -Params @{
      width             = 1560
      height            = 960
      deviceScaleFactor = 1
      mobile            = $false
    })

  Wait-ForCondition -Client $client -Condition "document.querySelector('.nav button') && document.querySelector('.topbar h2')" -Description "desktop shell"
  [void](Invoke-JsValue -Client $client -Expression "document.fonts ? document.fonts.ready.then(() => true) : true" -AwaitPromise)

  Prepare-QuickSwapState -Client $client

  Open-Page -Client $client -Label "Home"
  Scroll-ToTop -Client $client
  Save-Screenshot -Client $client -FileName "home.png"

  Open-Page -Client $client -Label "Game Folder"
  Scroll-ToTop -Client $client
  Save-Screenshot -Client $client -FileName "game_folder.png"

  Open-Page -Client $client -Label "Quick Swap"
  Scroll-ToTop -Client $client
  Save-Screenshot -Client $client -FileName "quick_swap.png"

  Open-Page -Client $client -Label "Install Preview"
  Scroll-ToSelector -Client $client -Selector ".install-files-panel"
  Save-Screenshot -Client $client -FileName "install_preview.png"

  Open-Page -Client $client -Label "Active Swaps"
  if (Click-FirstRowAction -Client $client) {
    Wait-ForCondition -Client $client -Condition "document.body.innerText.includes('source_decal_on_target_decal')" -Description "selected swap"
    Click-Button -Client $client -Label "Load restore preview"
    Wait-ForCondition -Client $client -Condition "document.body.innerText.includes('RESTORE source_decal_on_target_decal') || document.body.innerText.toLowerCase().includes('phrase matched') || document.body.innerText.toLowerCase().includes('type exact phrase')" -Description "restore preview"
  }
  Scroll-ToTop -Client $client
  Save-Screenshot -Client $client -FileName "active_swaps.png"

  Open-Page -Client $client -Label "Backups"
  Scroll-ToTop -Client $client
  Save-Screenshot -Client $client -FileName "backups.png"

  Open-Page -Client $client -Label "Diagnostics"
  Scroll-ToTop -Client $client
  Save-Screenshot -Client $client -FileName "diagnostics.png"

  Open-Page -Client $client -Label "Logs"
  Scroll-ToTop -Client $client
  Save-Screenshot -Client $client -FileName "logs.png"
}
finally {
  $client.Dispose()
}