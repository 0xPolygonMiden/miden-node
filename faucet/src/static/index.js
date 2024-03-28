window.onload = function() {
    fetch('http://localhost:8080/faucet_metadata')
        .then(response => response.json())
        .then(data => {
            document.getElementById('faucetId').textContent = data.id;
            document.getElementById('button').textContent = `send me ${data.asset_amount} tokens!`;
        })
        .catch(error => {
            console.error('Error fetching Faucet ID:', error);
            document.getElementById('faucetId').textContent = 'Error loading Faucet ID.';
            console.error('Error fetching Faucet asset amount:', error);
            document.getElementById('button').textContent = 'Error retrieving Faucet asset amount.';
        });
};

document.addEventListener('DOMContentLoaded', function () {
    let button = document.getElementById('button');
    let accountIdInput = document.getElementById('account-id');
    let errorMessage = document.getElementById('error-message');

    button.addEventListener('click', function () {
        let accountId = accountIdInput.value;
        errorMessage.style.display = 'none';

        let isValidAccountId = /^0x[0-9a-fA-F]{16}$/i.test(accountId);

        if (!accountId) {
            // Display the error message and prevent the fetch call
            errorMessage.textContent = "Account ID is required."
            errorMessage.style.display = 'block';
        } else if (!isValidAccountId) {
            // Display the error message and prevent the fetch call
            errorMessage.textContent = "Invalid Account ID."
            errorMessage.style.display = 'block';
        } else {
            fetch(`http://localhost:8080/get_tokens?account_id=${accountId}`)
                .then(response => {
                    if (!response.ok) {
                        console.log(response.text)
                        throw new Error(`HTTP error! status: ${response.status}`);
                    }
                    return response.blob(); // Handle the response as a blob instead of JSON
                })
                .then(blob => {
                    // Create a URL for the blob
                    const url = window.URL.createObjectURL(blob);
                    // Create a link element
                    const a = document.createElement('a');
                    a.style.display = 'none';
                    a.href = url;
                    a.download = 'note.mno'; // Provide a filename for the download
                    document.body.appendChild(a);
                    a.click();
                    window.URL.revokeObjectURL(url); // Clean up the URL object
                })
                .catch(error => {
                    console.log(error);
                    console.error('Error:', error);
                });
        }
    });
});
