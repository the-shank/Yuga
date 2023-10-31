<?php

function deleteDirectory($dir) {
    system('rm -rf -- ' . escapeshellarg($dir), $retval);
    return $retval == 0; // UNIX commands return zero on success
}

if ($_SERVER["REQUEST_METHOD"] === "POST") {

    $url 	= filter_var($_POST["url"], FILTER_SANITIZE_URL);
    $hash 	= $_POST["hash"];
    $subdir = $_POST["subdir"];

	if ($subdir == "") {
		$subdir = ".";
	}
	if ($hash == "") {
		$hash = "HEAD";
	}

    if (is_dir("/var/www/html/yuga_reports")) {
    	deleteDirectory("/var/www/html/yuga_reports");
	}
    $command = "bash ./run-yuga.sh " . escapeshellarg($url) . " /var/www/html/ " . escapeshellarg($hash) . " " . escapeshellarg($subdir) . " 2>&1";

	while (@ ob_end_flush()); // end all output buffers if any

	$proc = popen($command, 'r');
	echo '<pre>';
	while (!feof($proc)) {
	    echo fread($proc, 4096);
	    @ flush();
	}
	echo '</pre>';

	echo '__reports__';

	$basedir = "/var/www/html/yuga_reports/";

	if (is_dir($basedir)) {
		$files_list = array_filter(scandir($basedir), function($item) use ($basedir) {
    					return !is_dir($basedir . $item);
					});
		foreach($files_list as $filename) {
			echo "yuga_reports/" . $filename . "\n";
		}
	}
}
?>
