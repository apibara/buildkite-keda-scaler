# Buildkite Keda Scaler

This repo implements an [external Keda scaler](https://keda.sh/docs/latest/concepts/external-scalers/).


## Usage

```yaml
apiVersion: keda.sh/v1alpha1
kind: ScaledObject
metadata:
  name: scaledobject-name
  namespace: scaledobject-namespace
spec:
  scaleTargetRef:
    name: deployment-name
  triggers:
    - type: external
      metadata:
        scalerAddress: buldkite-scaler:9090
        queue: default
```

