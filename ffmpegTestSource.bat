ffmpeg -loglevel debug -f dshow -i video="UHD 4K Camera" -filter_complex "[0:v]scale=480:320,eq=brightness=-0.2:contrast=0.6,split=9[in1][in1][in1][in1][in1][in1][in1][in1][in1];"^
"[in1]drawtext=text=1%{gmtime\\:%X.%N}:fontfile='C\:/Windows/Fonts/Arial.ttf':fontsize=32:fontcolor=red[out1];"^
"[in2]drawtext=text=2%{gmtime\\:%X.%N}:fontfile='C\:/Windows/Fonts/Arial.ttf':fontsize=32:fontcolor=red[out2];"^
"[in3]drawtext=text=3%{gmtime\\:%X.%N}:fontfile='C\:/Windows/Fonts/Arial.ttf':fontsize=32:fontcolor=red[out3];"^
"[in4]drawtext=text=4%{gmtime\\:%X.%N}:fontfile='C\:/Windows/Fonts/Arial.ttf':fontsize=32:fontcolor=red[out4];"^
"[in5]drawtext=text=5%{gmtime\\:%X.%N}:fontfile='C\:/Windows/Fonts/Arial.ttf':fontsize=32:fontcolor=red[out5];"^
"[in6]drawtext=text=6%{gmtime\\:%X.%N}:fontfile='C\:/Windows/Fonts/Arial.ttf':fontsize=32:fontcolor=red[out6];"^
"[in7]drawtext=text=7%{gmtime\\:%X.%N}:fontfile='C\:/Windows/Fonts/Arial.ttf':fontsize=32:fontcolor=red[out7];"^
"[in8]drawtext=text=8%{gmtime\\:%X.%N}:fontfile='C\:/Windows/Fonts/Arial.ttf':fontsize=32:fontcolor=red[out8];"^
"[in9]drawtext=text=9%{gmtime\\:%X.%N}:fontfile='C\:/Windows/Fonts/Arial.ttf':fontsize=32:fontcolor=red[out9]" ^
-map "[out1]" -vcodec libvpx -cpu-used 5 -deadline 1 -g 10 -error-resilient 1 -auto-alt-ref 1 -f rtp -b:v 512k -maxrate 524k -v 0 rtp://127.0.0.1:5004?pkt_size=1200 ^
-map "[out2]" -vcodec libvpx -cpu-used 5 -deadline 1 -g 10 -error-resilient 1 -auto-alt-ref 1 -f rtp -b:v 512k -maxrate 524k -v 0 rtp://127.0.0.1:5004?pkt_size=1200 ^
-map "[out3]" -vcodec libvpx -cpu-used 5 -deadline 1 -g 10 -error-resilient 1 -auto-alt-ref 1 -f rtp -b:v 512k -maxrate 524k -v 0 rtp://127.0.0.1:5004?pkt_size=1200 ^
-map "[out4]" -vcodec libvpx -cpu-used 5 -deadline 1 -g 10 -error-resilient 1 -auto-alt-ref 1 -f rtp -b:v 512k -maxrate 524k -v 0 rtp://127.0.0.1:5004?pkt_size=1200 ^
-map "[out5]" -vcodec libvpx -cpu-used 5 -deadline 1 -g 10 -error-resilient 1 -auto-alt-ref 1 -f rtp -b:v 512k -maxrate 524k -v 0 rtp://127.0.0.1:5004?pkt_size=1200 ^
-map "[out6]" -vcodec libvpx -cpu-used 5 -deadline 1 -g 10 -error-resilient 1 -auto-alt-ref 1 -f rtp -b:v 512k -maxrate 524k -v 0 rtp://127.0.0.1:5004?pkt_size=1200 ^
-map "[out7]" -vcodec libvpx -cpu-used 5 -deadline 1 -g 10 -error-resilient 1 -auto-alt-ref 1 -f rtp -b:v 512k -maxrate 524k -v 0 rtp://127.0.0.1:5004?pkt_size=1200 ^
-map "[out8]" -vcodec libvpx -cpu-used 5 -deadline 1 -g 10 -error-resilient 1 -auto-alt-ref 1 -f rtp -b:v 512k -maxrate 524k -v 0 rtp://127.0.0.1:5004?pkt_size=1200 ^
-map "[out9]" -vcodec libvpx -cpu-used 5 -deadline 1 -g 10 -error-resilient 1 -auto-alt-ref 1 -f rtp -b:v 512k -maxrate 524k -v 0 rtp://127.0.0.1:5004?pkt_size=1200