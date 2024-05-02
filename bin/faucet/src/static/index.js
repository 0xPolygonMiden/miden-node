document.addEventListener('DOMContentLoaded', function () {
    const faucetIdElem = document.getElementById('faucetId');
    const button = document.getElementById('button');
    const privateButton = document.getElementById('button-private');
    const publicButton = document.getElementById('button-public');
    const accountIdInput = document.getElementById('account-id');
    const errorMessage = document.getElementById('error-message');
    const infoContainer = document.getElementById('info-container');
    const importCommand = document.getElementById('import-command');
    const noteIdElem = document.getElementById('note-id');
    const accountIdElem = document.getElementById('command-account-id');
    let isPrivateNote = true;

    fetchMetadata();

    button.addEventListener('click', handleButtonClick);
    privateButton.textContent = 'Private Note';
    publicButton.textContent = 'Public Note';
    privateButton.addEventListener('click', () => {toggleVisibility(true)});
    publicButton.addEventListener('click', () => {toggleVisibility(false)});

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
        infoContainer.style.display = 'none';
        importCommand.style.display = 'none';

        try {
            const response = await fetch('http://localhost:8080/get_tokens', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ account_id: accountId, is_private_note: isPrivateNote})
            });

            if (!response.ok) {
                throw new Error(`HTTP error! Status: ${response.status}`);
            }

            const blob = await response.blob();
            if(isPrivateNote) {
                importCommand.style.display = 'block';
                downloadBlob(blob, 'note.mno');
            }

            const noteId = response.headers.get('Content-Disposition').split('filename=')[1].replace(/"/g, '');
            noteIdElem.textContent = noteId;
            accountIdElem.textContent = accountId;
            infoContainer.style.display = 'block';
        } catch (error) {
            console.error('Error:', error);
            errorMessage.textContent = 'Failed to receive tokens. Please try again.';
            errorMessage.style.display = 'block';
        } finally {
            button.textContent = button.dataset.originalText;
        }
    }

    function toggleVisibility(clickedPrivate) {
        if (clickedPrivate) {
            privateButton.classList.add('active');
            privateButton.classList.remove('inactive');
            publicButton.classList.remove('active');
            publicButton.classList.add('inactive');
        } else {
            publicButton.classList.add('active');
            publicButton.classList.remove('inactive');
            privateButton.classList.remove('active');
            privateButton.classList.add('inactive');
        }
        isPrivateNote = clickedPrivate;
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
