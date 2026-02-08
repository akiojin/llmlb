# E2E Full Walkthrough Test Documentation

This document describes all screens in the LLM Load Balancer web application and the actions
required for a complete end-to-end walkthrough test.

## Application Structure

```text
LLM Load Balancer Web Application
├── Login Page (/dashboard/login.html)
├── Dashboard (/dashboard/)
│   ├── Header
│   │   ├── Logo
│   │   ├── API Keys Button → API Keys Modal
│   │   ├── Refresh Button
│   │   ├── Theme Toggle
│   │   └── User Menu → User Modal (admin only)
│   ├── Stats Cards
│   └── Tabs
│       ├── Endpoints Tab
│       ├── Models Tab
│       ├── History Tab
│       └── Logs Tab
└── Playground (/dashboard/#playground/:endpointId)
    ├── Sidebar (Endpoint info)
    ├── Chat Area
    ├── Settings Dialog
    └── curl Dialog
```

## Screen-by-Screen Walkthrough Requirements

### 1. Login Page

**URL**: `/dashboard/login.html`

**Walkthrough Actions**:

| Step | Action | Verification |
|------|--------|--------------|
| 1.1 | Navigate to login page | Page title contains "Login" |
| 1.2 | Enter username "admin" | Input field accepts text |
| 1.3 | Enter password "test" | Input field accepts text |
| 1.4 | Click login button | Redirects to dashboard |

### 2. Dashboard - Header

**URL**: `/dashboard/`

**Walkthrough Actions**:

| Step | Action | Verification |
|------|--------|--------------|
| 2.1 | Verify header elements visible | Logo, buttons present |
| 2.2 | Click Theme Toggle | Theme changes (dark/light) |
| 2.3 | Click API Keys button | API Keys modal opens |
| 2.4 | Close API Keys modal | Modal closes |
| 2.5 | Click User Menu | Dropdown opens |
| 2.6 | Click "Manage Users" (admin) | User modal opens |
| 2.7 | Close User modal | Modal closes |

### 3. Dashboard - Stats Cards

**Walkthrough Actions**:

| Step | Action | Verification |
|------|--------|--------------|
| 3.1 | View stats cards | Cards display node count, request stats |
| 3.2 | Verify data loading | Loading state completes |

### 4. Dashboard - Endpoints Tab

**Walkthrough Actions**:

| Step | Action | Verification |
|------|--------|--------------|
| 4.1 | Click Endpoints tab | Tab becomes active |
| 4.2 | View endpoints table | Table displays registered endpoints |
| 4.3 | Verify endpoint status | Status indicators visible |
| 4.4 | Test search/filter (if available) | Filter works |

### 5. Dashboard - Models Tab

**Walkthrough Actions**:

| Step | Action | Verification |
|------|--------|--------------|
| 5.1 | Click Models tab | Tab becomes active |
| 5.2 | View Registered sub-tab | Registered models displayed |
| 5.3 | Verify model information | Name, status, path visible |
| 5.4 | Click Available sub-tab | Available HF models listed |
| 5.5 | Click Register button | Register Model dialog opens |
| 5.6 | Fill Repo field | Input accepts HF repo path |
| 5.7 | Fill Filename field (optional) | Input accepts GGUF filename |
| 5.8 | Click Register in dialog | Toast shows "Model registration queued" |
| 5.9 | Click Convert Tasks sub-tab | Task progress displayed |
| 5.10 | Wait for task completion | Status changes to "Completed" |

### 6. Dashboard - History Tab

**Walkthrough Actions**:

| Step | Action | Verification |
|------|--------|--------------|
| 6.1 | Click History tab | Tab becomes active |
| 6.2 | View request history | History table displayed |
| 6.3 | Test pagination (if available) | Pagination works |

### 7. Dashboard - Logs Tab

**Walkthrough Actions**:

| Step | Action | Verification |
|------|--------|--------------|
| 7.1 | Click Logs tab | Tab becomes active |
| 7.2 | View log viewer | Logs displayed |
| 7.3 | Test log filtering (if available) | Filter works |

### 8. Playground - Navigation

**URL**: `/dashboard/#playground/:endpointId`

**Walkthrough Actions**:

| Step | Action | Verification |
|------|--------|--------------|
| 8.1 | Open an endpoint's detail view | Detail modal opens |
| 8.2 | Navigate to Playground for that endpoint | Playground view is shown |
| 8.3 | Verify playground loads | Chat interface visible |

### 9. Playground - Model Selection

**Walkthrough Actions**:

| Step | Action | Verification |
|------|--------|--------------|
| 9.1 | Click model selector | Dropdown opens |
| 9.2 | View available models | Models listed |
| 9.3 | Select a model | Model is selected |

### 10. Playground - Chat (All Models)

**Important**: Test EVERY registered model, not just one. If any model fails, the test fails.

**Walkthrough Actions** (repeat for each model):

| Step | Action | Verification |
|------|--------|--------------|
| 10.1 | Select model from dropdown | Model name shown in selector |
| 10.2 | Click "New Chat" | Fresh chat session starts |
| 10.3 | Type test message in input | Text appears |
| 10.4 | Press Enter or click Send | Message sent |
| 10.5 | Wait for response | LLM response received (no error) |
| 10.6 | Verify response displayed | Non-empty response in chat |
| 10.7 | Repeat 10.1-10.6 for next model | All models tested |

### 11. Playground - Sessions

**Walkthrough Actions**:

| Step | Action | Verification |
|------|--------|--------------|
| 11.1 | Create new session | New session appears |
| 11.2 | Switch between sessions | Chat context changes |
| 11.3 | Delete session | Session removed |

### 12. Playground - Settings

**Walkthrough Actions**:

| Step | Action | Verification |
|------|--------|--------------|
| 12.1 | Open settings dialog | Dialog opens |
| 12.2 | Modify temperature | Slider updates |
| 12.3 | Modify max tokens | Input updates |
| 12.4 | Close settings | Settings saved |

## Test Execution Order

For a complete walkthrough, execute in this order:

1. Login (1.1-1.4)
2. Dashboard Header (2.1-2.7)
3. Stats Cards (3.1-3.2)
4. Endpoints Tab (4.1-4.4)
5. Models Tab with Registration (5.1-5.10)
6. History Tab (6.1-6.3)
7. Logs Tab (7.1-7.3)
8. Open Playground (8.1-8.3)
9. Model Selection (9.1-9.3)
10. Chat Test - ALL Models (10.1-10.7 for each model)
11. Sessions (11.1-11.3)
12. Settings (12.1-12.4)

## Test Data

- **Login Credentials**: `admin` / `test` (development mode)
- **API Key**: `sk_debug` (development mode)
- **Test Models**: ALL models from `/api/models` endpoint
- **Test Message**: "Hello, this is a test message"
- **Sample HF Repo**: `Qwen/Qwen2.5-0.5B-Instruct-GGUF`
- **Sample GGUF File**: `qwen2.5-0.5b-instruct-q4_k_m.gguf`

## Critical Test Requirements

1. **All Models Must Be Tested**: Do not skip any model. If a model fails, investigate
   the root cause rather than switching to another model.
2. **Model Registration Flow**: The registration dialog and Convert Tasks must be verified.
3. **No Silent Failures**: Every API call should return 200 OK for success paths.
