document.addEventListener('DOMContentLoaded', function () {
    let button = document.getElementById('button');
    let accountIdInput = document.getElementById('account-id');

    button.addEventListener('click', function () {
        let accountId = accountIdInput.value;

        if (accountId) {
            let requestOptions = {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ account_id: accountId }),
            };

            fetch('http://127.0.0.1:8080/get_tokens', requestOptions)
                .then(response => {
                    if (!response.ok) {
                        throw new Error(`HTTP error! status: ${response.status}`);
                    }
                    return response.json();
                })
                .then(result => {
                    console.log(result);
                })
                .catch(error => {
                    console.error('Error:', error);
                });
        } else {
            console.error('Account ID is required');
        }
    });
});
