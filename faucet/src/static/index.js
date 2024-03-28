document.addEventListener('DOMContentLoaded', function () {
    const faucetIdElem = document.getElementById('faucetId');
    const button = document.getElementById('button');
    const accountIdInput = document.getElementById('account-id');
    const errorMessage = document.getElementById('error-message');

    fetchMetadata();

    button.addEventListener('click', handleButtonClick);

    function fetchMetadata() {
        fetch('http://localhost:8080/get_metadata')
            .then(response => response.json())
            .then(data => {
                faucetIdElem.textContent = data.id;
                button.textContent = `Send me ${data.asset_amount} tokens!`;
                button.dataset.originalText = button.textContent;
            })
            .catch(error => {
                console.error('Error fetching metadata:', error);
                faucetIdElem.textContent = 'Error loading Faucet ID.';
                button.textContent = 'Error retrieving Faucet asset amount.';
            });
    }

    async function handleButtonClick() {
        let accountId = accountIdInput.value.trim();
        errorMessage.style.display = 'none';

        if (!accountId || !/^0x[0-9a-fA-F]{16}$/i.test(accountId)) {
            errorMessage.textContent = !accountId ? "Account ID is required." : "Invalid Account ID.";
            errorMessage.style.display = 'block';
            return;
        }

        button.textContent = 'Loading...';
        try {
            const response = await fetch('http://localhost:8080/get_tokens', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ account_id: accountId })
            });

            if (!response.ok) {
                throw new Error(`HTTP error! Status: ${response.status}`);
            }

            const blob = await response.blob();
            downloadBlob(blob, 'note.mno');
        } catch (error) {
            console.error('Error:', error);
            errorMessage.textContent = 'Failed to receive tokens. Please try again.';
            errorMessage.style.display = 'block';
        } finally {
            button.textContent = button.dataset.originalText;
        }
    }

    function downloadBlob(blob, filename) {
        const url = window.URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.style.display = 'none';
        a.href = url;
        a.download = filename;
        document.body.appendChild(a);
        a.click();
        a.remove();
        window.URL.revokeObjectURL(url);
    }
});
