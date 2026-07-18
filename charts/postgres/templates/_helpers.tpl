{{/*
Expand the name of the chart.
*/}}
{{- define "postgres.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "postgres.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{- define "postgres.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "postgres.labels" -}}
helm.sh/chart: {{ include "postgres.chart" . }}
{{ include "postgres.selectorLabels" . }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end }}

{{- define "postgres.selectorLabels" -}}
app.kubernetes.io/name: {{ include "postgres.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{- define "postgres.primaryName" -}}
{{- .Values.primary.name | default "postgresql-primary" }}
{{- end }}

{{- define "postgres.serviceName" -}}
{{- if .Values.service.name }}
{{- .Values.service.name }}
{{- else }}
{{- include "postgres.primaryName" . }}
{{- end }}
{{- end }}

{{- define "postgres.secretName" -}}
{{- if .Values.auth.existingSecret }}
{{- .Values.auth.existingSecret }}
{{- else }}
{{- printf "%s-credentials" (include "postgres.fullname" .) }}
{{- end }}
{{- end }}

{{- define "postgres.initConfigMapName" -}}
{{- printf "%s-init" (include "postgres.fullname" .) }}
{{- end }}

{{- define "postgres.masterHost" -}}
{{- if .Values.replica.masterHost }}
{{- .Values.replica.masterHost }}
{{- else }}
{{- include "postgres.serviceName" . }}.{{ .Release.Namespace }}.svc.cluster.local
{{- end }}
{{- end }}

{{- define "postgres.image" -}}
{{- printf "%s/%s:%s" .Values.image.registry .Values.image.repository .Values.image.tag }}
{{- end }}

{{- define "postgres.exporterImage" -}}
{{- printf "%s/%s:%s" .Values.exporter.image.registry .Values.exporter.image.repository .Values.exporter.image.tag }}
{{- end }}

{{- define "postgres.dataSourceName" -}}
{{- if .Values.exporter.dataSourceName }}
{{- .Values.exporter.dataSourceName }}
{{- else }}
{{- printf "postgresql://%s:%s@%s:%v/%s?sslmode=disable" .Values.auth.username .Values.auth.password (include "postgres.serviceName" .) .Values.service.port .Values.auth.database }}
{{- end }}
{{- end }}
