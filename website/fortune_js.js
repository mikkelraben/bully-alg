// Fortune cookie dummy beskeder
const fortunes = [
    "Today it's up to you to create the peacefulness you long for.",
    "A friend asks only for your time not your money.",
    "If you refuse to accept anything but the best, you very often get it.",
    "A smile is your passport into the hearts of others.",
    "A good way to keep healthy is to eat more Chinese food.",
    "Your high-minded principles spell success.",
    "Hard work pays off in the future, laziness pays off now.",
    "Change can hurt, but it leads a path to something better.",
    "Enjoy the good luck a companion brings you.",
    "People are naturally attracted to you.",
    "Hidden in a valley beside an open stream- This will be the type of place where you will find your dream.",
    "A chance meeting opens new doors to success and friendship.",
    "You learn from your mistakes... You will learn a lot today.",
    "If you have something good in your life, don't let it go!",
    "What ever you're goal is in life, embrace it visualize it, and for it will be yours.",
    "Your shoes will make you happy today.",
    "You cannot love life until you live the life you love.",
    "Be on the lookout for coming events; They cast their shadows beforehand.",
    "Land is always on the mind of a flying bird.",
    "The man or woman you desire feels the same about you.",
    "Meeting adversity well is the source of your strength.",
    "A dream you have will come true.",
    "Our deeds determine us, as much as we determine our deeds.",
    "Never give up. You're not a failure if you don't give up.",
    "You will become great if you believe in yourself.",
    "There is no greater pleasure than seeing your loved ones prosper.",
    "You will marry your lover.",
    "A very attractive person has a message for you.",
    "You already know the answer to the questions lingering inside your head.",
    "It is now, and in this world, that we must live.",
    "You must try, or hate yourself for not trying.",
    "You can make your own happiness."
];

// Hent elementer
const fortuneText = document.getElementById('fortune-text');
const fortuneButton = document.getElementById('fortune-button');

// Funktion til at få en tilfældig fortune
function getRandomFortune() {
    const randomIndex = Math.floor(Math.random() * fortunes.length);
    return fortunes[randomIndex];
}

// Funktion til at opdatere fortune teksten
function updateFortune() {
    // Vis loading tekst
    fortuneText.textContent = "Getting your fortune...";
    
    // Simuler en lille delay (som om vi kalder API)
    setTimeout(() => {
        const newFortune = getRandomFortune();
        fortuneText.textContent = `"${newFortune}"`;
    }, 300);
}

// Tilføj event listener til knappen

fortuneButton.addEventListener('click', updateFortune);

// ============================================
// KUBERNETES VERSION (udkommenteret - brug når Rust API er klar)
// ============================================



/*
// API endpoint - skift til din Kubernetes service URL
const API_URL = 'http://localhost:8080'; // eller din NodePort URL

// Funktion til at hente fortune fra Rust API
async function getFortune() {
    try {
        fortuneText.textContent = "Getting your fortune...";
        
        // Kald /fortune endpoint (skal tilføjes i Rust)
        const response = await fetch(`${API_URL}/fortune`);
        
        if (!response.ok) {
            throw new Error('Failed to get fortune');
        }
        
        const data = await response.text();
        fortuneText.textContent = `"${data}"`;
        
    } catch (error) {
        console.error('Error fetching fortune:', error);
        fortuneText.textContent = "Error: Could not connect to server. Using local fortune...";
        
        // Fallback til lokal fortune
        setTimeout(() => {
            const newFortune = getRandomFortune();
            fortuneText.textContent = `"${newFortune}"`;
        }, 1000);
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

// Brug denne når Rust API er klar:
// fortuneButton.addEventListener('click', getFortune);

// Hent state ved page load (til debugging)
// getState();
*/
