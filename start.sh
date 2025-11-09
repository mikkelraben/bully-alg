docker build -t leader-election .
kubectl apply -f ./kubernetes/get-cookies-deployment.yaml
kubectl apply -f ./kubernetes/get-pods-service.yaml
kubectl rollout restart deployment.apps/leader-election-deployment