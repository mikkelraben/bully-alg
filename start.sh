eval $(minikube docker-env)
docker build -t leader-election .
kubectl apply -f ./kubernetes/get-cookies-deployment.yaml
kubectl apply -f ./kubernetes/get-pods-service.yaml
kubectl apply -f ./kubernetes/website-service.yaml
kubectl apply -f ./kubernetes/permissions.yaml
kubectl rollout restart deployment.apps/leader-election-deployment
kubectl expose service website-service --type=NodePort --port=8080
minikube service website-service --url