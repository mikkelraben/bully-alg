// Hent elementer
const fortuneText = document.getElementById('fortune-text');
const fortuneButton = document.getElementById('fortune-button');

// API endpoint - bruger relative path så det virker både lokalt og i Kubernetes
const API_URL = window.location.origin;

// Funktion til at hente fortune fra Rust API
async function getFortune() {
    try {
        fortuneText.textContent = "Getting your fortune...";
        
        // Kald /fortune endpoint
        const response = await fetch(`${API_URL}/fortune`);
        
        if (!response.ok) {
            throw new Error('Failed to get fortune');
        }
        
        const data = await response.text();
        fortuneText.textContent = `"${data}"`;
        
    } catch (error) {
        console.error('Error fetching fortune:', error);
        fortuneText.textContent = "Error: Could not connect to server 😢";
    }
}

// Funktion til at hente state fra Rust API (til debugging)
async function getState() {
    try {
        const response = await fetch(`${API_URL}/state`);
        const data = await response.json();
        console.log('Current state:', data);
        console.log('Coordinator:', data.coordinator);
        console.log('Local pod:', data.local_pod);
    } catch (error) {
        console.error('Error fetching state:', error);
    }
}

// Event listener til knappen
fortuneButton.addEventListener('click', getFortune);

// Hent state ved page load (til debugging - kan udkommenteres)
getState();
