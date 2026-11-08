{{- define "worldwright.name" -}}{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}{{- end -}}
{{- define "worldwright.fullname" -}}{{- printf "%s-%s" .Release.Name (include "worldwright.name" .) | trunc 63 | trimSuffix "-" -}}{{- end -}}
{{- define "worldwright.labels" -}}
app.kubernetes.io/name: {{ include "worldwright.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end -}}
{{- define "worldwright.serviceAccountName" -}}{{- if .Values.serviceAccount.create }}{{ include "worldwright.fullname" . }}{{ else }}{{ .Values.serviceAccount.name | quote }}{{ end }}{{- end -}}
